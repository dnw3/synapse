//! Discord bot adapter.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use colored::Colorize;
use futures::{SinkExt, StreamExt};
use serde_json::json;
use tokio_tungstenite::tungstenite::Message as WsMsg;
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

/// Outbound sender for the Discord channel.
#[allow(dead_code)]
pub struct DiscordSender {
    /// HTTP client for making Discord API calls.
    pub client: reqwest::Client,
    /// Discord bot token (without the "Bot " prefix).
    pub token: String,
}

#[async_trait]
impl ChannelSender for DiscordSender {
    fn channel_id(&self) -> &str {
        "discord"
    }

    async fn send(
        &self,
        target: &DeliveryContext,
        content: &str,
        _meta: Option<&serde_json::Value>,
    ) -> Result<SendResult, Box<dyn std::error::Error + Send + Sync>> {
        let channel_id = target
            .to
            .as_deref()
            .and_then(|s| s.strip_prefix("channel:"))
            .ok_or("missing or invalid channel_id in delivery target (expected 'channel:<id>')")?;

        let chunks = formatter::format_for_channel(content, "discord", 2000);
        let mut last_message_id: Option<String> = None;
        for chunk in chunks {
            let resp: serde_json::Value = self
                .client
                .post(format!(
                    "https://discord.com/api/v10/channels/{}/messages",
                    channel_id
                ))
                .header("Authorization", format!("Bot {}", self.token))
                .json(&json!({"content": chunk}))
                .send()
                .await?
                .json()
                .await?;
            if let Some(msg_id) = resp.get("id").and_then(|v| v.as_str()) {
                last_message_id = Some(msg_id.to_string());
            }
        }

        Ok(SendResult {
            message_id: last_message_id,
            delivered_at_ms: now_ms(),
        })
    }
}

/// Run the Discord bot.
pub async fn run(
    config: &SynapseConfig,
    model_override: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let discord_config = config
        .discord
        .first()
        .ok_or("Discord bot configuration not found in config")?;

    let token = resolve_secret(
        discord_config.bot_token.as_deref(),
        discord_config.bot_token_env.as_deref(),
        "Discord bot token",
    )
    .map_err(|e| e.to_string())?;

    let model = agent::build_model(config, model_override)?;
    let config_arc = Arc::new(config.clone());
    let allowlist = discord_config.allowlist.clone();
    let agent_session =
        Arc::new(AgentSession::new(model, config_arc, true).with_channel("discord"));

    if !allowlist.is_empty() {
        eprintln!(
            "{} Allowlist active ({} users, {} channels)",
            "discord:".blue().bold(),
            allowlist.allowed_users.len(),
            allowlist.allowed_channels.len()
        );
    }

    eprintln!(
        "{}",
        "Starting Discord bot (Deep Agent mode)...".green().bold()
    );

    // Get gateway URL
    let client = reqwest::Client::new();
    let gateway_resp: serde_json::Value = client
        .get("https://discord.com/api/v10/gateway/bot")
        .header("Authorization", format!("Bot {}", token))
        .send()
        .await?
        .json()
        .await?;

    let gateway_url = gateway_resp["url"]
        .as_str()
        .ok_or("failed to get gateway URL")?;
    let ws_url = format!("{}/?v=10&encoding=json", gateway_url);

    let (ws_stream, _) = tokio_tungstenite::connect_async(&ws_url).await?;
    let (mut write, mut read) = ws_stream.split();

    // Read the Hello event to get heartbeat interval
    let hello = read.next().await.ok_or("no hello from gateway")??;
    let hello_data: serde_json::Value = serde_json::from_str(hello.to_text().unwrap_or("{}"))?;
    let heartbeat_interval = hello_data["d"]["heartbeat_interval"]
        .as_u64()
        .unwrap_or(45000);

    // Send Identify
    let identify = json!({
        "op": 2,
        "d": {
            "token": token,
            "intents": 33281, // GUILDS | GUILD_MESSAGES | MESSAGE_CONTENT
            "properties": {
                "os": "linux",
                "browser": "synapse",
                "device": "synapse"
            }
        }
    });
    write.send(WsMsg::Text(identify.to_string().into())).await?;

    // Spawn heartbeat task
    let heartbeat_interval_ms = heartbeat_interval;
    let (heartbeat_tx, heartbeat_rx) = tokio::sync::watch::channel(serde_json::Value::Null);
    let mut heartbeat_write = write;

    let heartbeat_handle = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_millis(heartbeat_interval_ms));
        loop {
            interval.tick().await;
            let seq = heartbeat_rx.borrow().clone();
            let hb = json!({"op": 1, "d": seq});
            if heartbeat_write
                .send(WsMsg::Text(hb.to_string().into()))
                .await
                .is_err()
            {
                break;
            }
        }
    });

    // Process events
    let http_client = reqwest::Client::new();
    let token_clone = token.clone();

    eprintln!("{}", "Discord bot connected.".green());

    while let Some(msg) = read.next().await {
        let msg = match msg {
            Ok(m) => m,
            Err(_) => break,
        };
        let text = match msg.to_text() {
            Ok(t) => t,
            Err(_) => continue,
        };
        let event: serde_json::Value = match serde_json::from_str(text) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Update sequence number for heartbeat
        if let Some(seq) = event.get("s") {
            let _ = heartbeat_tx.send(seq.clone());
        }

        let op = event["op"].as_u64().unwrap_or(0);
        let event_type = event["t"].as_str().unwrap_or("");

        match (op, event_type) {
            (0, "MESSAGE_CREATE") => {
                let data = &event["d"];
                // Skip bot messages
                if data["author"]["bot"].as_bool().unwrap_or(false) {
                    continue;
                }
                let content = data["content"].as_str().unwrap_or("").to_string();
                let channel_id = data["channel_id"].as_str().unwrap_or("").to_string();
                let message_id = data["id"].as_str().unwrap_or("").to_string();
                let author_id = data["author"]["id"].as_str().unwrap_or("");
                let guild_id = data["guild_id"].as_str().map(|s| s.to_string());

                // Extract attachments (Discord gives direct CDN URLs)
                let mut attachments = Vec::new();
                if let Some(atts) = data["attachments"].as_array() {
                    for att in atts {
                        let filename = att["filename"].as_str().unwrap_or("file").to_string();
                        let url = att["url"].as_str().unwrap_or("").to_string();
                        let content_type = att["content_type"].as_str().map(|s| s.to_string());
                        if !url.is_empty() {
                            attachments.push(Attachment {
                                filename,
                                url,
                                mime_type: content_type,
                            });
                        }
                    }
                }

                if content.is_empty() && attachments.is_empty() {
                    continue;
                }

                // Allowlist check
                if !allowlist.is_allowed(Some(author_id), Some(&channel_id)) {
                    continue;
                }

                // Use AgentSession with persistent history + deep agent
                let session = agent_session.clone();
                let http = http_client.clone();
                let tok = token_clone.clone();
                let sender_id = author_id.to_string();
                tokio::spawn(async move {
                    let request_id = logging::generate_request_id();
                    let span = tracing::info_span!("channel_message",
                        request_id = %request_id,
                        channel = "discord",
                        sender = %sender_id,
                        platform_msg_id = %message_id,
                    );
                    let _guard = span.enter();
                    tracing::info!("processing discord message");

                    // React with eyes to indicate processing
                    reactions::discord_react(&tok, &channel_id, &message_id, "\u{1f440}").await;

                    // Send typing indicator
                    let _ = http
                        .post(format!(
                            "https://discord.com/api/v10/channels/{}/typing",
                            channel_id
                        ))
                        .header("Authorization", format!("Bot {}", tok))
                        .send()
                        .await;

                    let is_dm = guild_id.is_none();
                    let delivery = DeliveryContext {
                        channel: "discord".into(),
                        to: Some(format!("channel:{}", channel_id)),
                        account_id: guild_id.clone(),
                        ..Default::default()
                    };
                    let mut envelope =
                        MessageEnvelope::channel(channel_id.clone(), content, delivery);
                    envelope.attachments = attachments;
                    envelope.sender_id = Some(sender_id.clone());
                    envelope.routing.peer_kind = Some(if is_dm {
                        crate::config::PeerKind::Direct
                    } else {
                        crate::config::PeerKind::Group
                    });
                    envelope.routing.peer_id = Some(channel_id.clone());
                    envelope.routing.guild_id = guild_id;

                    match session.handle_message(envelope).await {
                        Ok(reply) => {
                            // Split long replies into chunks (Discord 2000 char limit)
                            let chunks =
                                formatter::format_for_channel(&reply.content, "discord", 2000);
                            for chunk in chunks {
                                let _ = http
                                    .post(format!(
                                        "https://discord.com/api/v10/channels/{}/messages",
                                        channel_id
                                    ))
                                    .header("Authorization", format!("Bot {}", tok))
                                    .json(&json!({"content": chunk}))
                                    .send()
                                    .await;
                            }
                            // React with checkmark on success
                            reactions::discord_react(&tok, &channel_id, &message_id, "\u{2705}")
                                .await;
                        }
                        Err(e) => {
                            eprintln!("Discord: handler error: {}", e);
                        }
                    }
                });
            }
            (11, _) => {
                // Heartbeat ACK — ignore
            }
            _ => {}
        }
    }

    heartbeat_handle.abort();
    Ok(())
}
