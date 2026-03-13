use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use colored::Colorize;
use tracing;

use synaptic::DeliveryContext;

use crate::agent;
use crate::channels::formatter;
use crate::channels::handler::AgentSession;
use crate::channels::reactions;
use crate::config::bot::resolve_secret;
use crate::config::SynapseConfig;
use crate::gateway::messages::sender::{ChannelSender, SendResult};
use crate::gateway::messages::{Attachment, MessageEnvelope};
use crate::gateway::presence::now_ms;
use crate::logging;

// ---------------------------------------------------------------------------
// ChannelSender implementation
// ---------------------------------------------------------------------------

/// Outbound sender for the Telegram channel.
pub struct TelegramSender {
    /// HTTP client for making API calls.
    pub client: reqwest::Client,
    /// Base URL: `https://api.telegram.org/bot{TOKEN}`.
    pub base_url: String,
}

#[async_trait]
impl ChannelSender for TelegramSender {
    fn channel_id(&self) -> &str {
        "telegram"
    }

    async fn send(
        &self,
        target: &DeliveryContext,
        content: &str,
        _meta: Option<&serde_json::Value>,
    ) -> Result<SendResult, Box<dyn std::error::Error + Send + Sync>> {
        let chat_id = target
            .to
            .as_deref()
            .and_then(|s| s.strip_prefix("chat:"))
            .ok_or("missing or invalid chat_id in delivery target (expected 'chat:<id>')")?;

        let chunks = formatter::chunk_telegram(content);
        let mut last_message_id: Option<String> = None;
        for chunk in chunks {
            let resp: serde_json::Value = self
                .client
                .post(&format!("{}/sendMessage", self.base_url))
                .json(&serde_json::json!({
                    "chat_id": chat_id,
                    "text": chunk,
                }))
                .send()
                .await?
                .json()
                .await?;
            if let Some(msg_id) = resp
                .get("result")
                .and_then(|r| r.get("message_id"))
                .and_then(|v| v.as_i64())
            {
                last_message_id = Some(msg_id.to_string());
            }
        }

        Ok(SendResult {
            message_id: last_message_id,
            delivered_at_ms: now_ms(),
        })
    }
}

/// Run the Telegram bot adapter using Long Polling.
pub async fn run(
    config: &SynapseConfig,
    model_override: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let tg_config = config
        .telegram
        .as_ref()
        .ok_or("missing [telegram] section in config")?;

    let bot_token = resolve_secret(
        tg_config.bot_token.as_deref(),
        tg_config.bot_token_env.as_deref(),
        "Telegram bot token",
    )
    .map_err(|e| format!("{}", e))?;

    let model = agent::build_model(config, model_override)?;
    let config_arc = Arc::new(config.clone());
    let allowlist = tg_config.allowlist.clone();
    let agent_session =
        Arc::new(AgentSession::new(model, config_arc, true).with_channel("telegram"));

    if !allowlist.is_empty() {
        eprintln!(
            "{} Allowlist active ({} users, {} channels)",
            "telegram:".blue().bold(),
            allowlist.allowed_users.len(),
            allowlist.allowed_channels.len()
        );
    }

    eprintln!(
        "{}",
        "Starting Telegram bot (Deep Agent mode, Long Polling)..."
            .green()
            .bold()
    );

    let client = reqwest::Client::new();
    let base_url = format!("https://api.telegram.org/bot{}", bot_token);

    let mut offset: i64 = 0;

    loop {
        // Get updates with long polling
        let url = format!("{}/getUpdates?offset={}&timeout=30", base_url, offset);

        let resp = match client.get(&url).send().await {
            Ok(r) => r,
            Err(e) => {
                eprintln!(
                    "{} Telegram polling error: {}",
                    "warning:".yellow().bold(),
                    e
                );
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        let body: serde_json::Value = match resp.json().await {
            Ok(v) => v,
            Err(e) => {
                eprintln!(
                    "{} Telegram response error: {}",
                    "warning:".yellow().bold(),
                    e
                );
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        let updates = body
            .get("result")
            .and_then(|v| v.as_array())
            .cloned()
            .unwrap_or_default();

        for update in updates {
            let update_id = update
                .get("update_id")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            offset = update_id + 1;

            let message = match update.get("message") {
                Some(m) => m,
                None => continue,
            };

            let text = message
                .get("text")
                .or_else(|| message.get("caption"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let chat_id = message
                .get("chat")
                .and_then(|c| c.get("id"))
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let message_id = message
                .get("message_id")
                .and_then(|v| v.as_i64())
                .unwrap_or(0);
            let user_id = message
                .get("from")
                .and_then(|f| f.get("id"))
                .and_then(|v| v.as_i64())
                .map(|id| id.to_string());

            // Extract photo/document attachments
            let mut attachments = Vec::new();

            // Photos: array of PhotoSize, pick the largest (last)
            if let Some(photos) = message.get("photo").and_then(|v| v.as_array()) {
                if let Some(largest) = photos.last() {
                    if let Some(file_id) = largest.get("file_id").and_then(|v| v.as_str()) {
                        // Resolve file_id to a download URL via getFile API
                        if let Ok(file_url) =
                            resolve_telegram_file(&client, &base_url, file_id).await
                        {
                            attachments.push(Attachment {
                                filename: format!(
                                    "photo_{}.jpg",
                                    file_id.chars().take(8).collect::<String>()
                                ),
                                url: file_url,
                                mime_type: Some("image/jpeg".to_string()),
                            });
                        }
                    }
                }
            }

            // Documents
            if let Some(doc) = message.get("document") {
                if let Some(file_id) = doc.get("file_id").and_then(|v| v.as_str()) {
                    let filename = doc
                        .get("file_name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("document")
                        .to_string();
                    let mime = doc
                        .get("mime_type")
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());
                    if let Ok(file_url) = resolve_telegram_file(&client, &base_url, file_id).await {
                        attachments.push(Attachment {
                            filename,
                            url: file_url,
                            mime_type: mime,
                        });
                    }
                }
            }

            // Skip messages with no text AND no attachments
            if text.is_empty() && attachments.is_empty() || chat_id == 0 {
                continue;
            }

            // Allowlist check
            if !allowlist.is_allowed(user_id.as_deref(), Some(&chat_id.to_string())) {
                continue;
            }

            // Process in background
            let session = agent_session.clone();
            let base = base_url.clone();
            let http = client.clone();
            let sender_id = user_id.clone().unwrap_or_default();
            tokio::spawn(async move {
                let request_id = logging::generate_request_id();
                let span = tracing::info_span!("channel_message",
                    request_id = %request_id,
                    channel = "telegram",
                    sender = %sender_id,
                    platform_msg_id = %message_id,
                );
                let _guard = span.enter();
                tracing::info!("processing telegram message");

                // React with eyes to indicate processing
                reactions::telegram_react(&base, chat_id, message_id, "\u{1f440}").await;

                // Send typing indicator
                let _ = http
                    .post(&format!("{}/sendChatAction", base))
                    .json(&serde_json::json!({
                        "chat_id": chat_id,
                        "action": "typing",
                    }))
                    .send()
                    .await;

                let delivery = DeliveryContext {
                    channel: "telegram".into(),
                    to: Some(format!("chat:{}", chat_id)),
                    ..Default::default()
                };
                let mut envelope = MessageEnvelope::channel(chat_id.to_string(), text, delivery);
                envelope.attachments = attachments;

                match session.handle_message(envelope).await {
                    Ok(reply) => {
                        // Split long replies into chunks
                        let chunks = formatter::chunk_telegram(&reply.content);
                        for chunk in chunks {
                            let _ = http
                                .post(&format!("{}/sendMessage", base))
                                .json(&serde_json::json!({
                                    "chat_id": chat_id,
                                    "text": chunk,
                                }))
                                .send()
                                .await;
                        }
                        // React with checkmark on success
                        reactions::telegram_react(&base, chat_id, message_id, "\u{2705}").await;
                    }
                    Err(e) => {
                        eprintln!("Telegram handler error: {}", e);
                    }
                }
            });
        }
    }
}

/// Resolve a Telegram file_id to a downloadable URL via the getFile API.
async fn resolve_telegram_file(
    client: &reqwest::Client,
    base_url: &str,
    file_id: &str,
) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
    let url = format!("{}/getFile?file_id={}", base_url, file_id);
    let resp: serde_json::Value = client.get(&url).send().await?.json().await?;
    let file_path = resp
        .get("result")
        .and_then(|r| r.get("file_path"))
        .and_then(|v| v.as_str())
        .ok_or("no file_path in getFile response")?;

    // Construct the download URL
    // base_url is like "https://api.telegram.org/bot<TOKEN>"
    // file download is "https://api.telegram.org/file/bot<TOKEN>/<file_path>"
    let download_url = base_url.replace("/bot", "/file/bot");
    Ok(format!("{}/{}", download_url, file_path))
}
