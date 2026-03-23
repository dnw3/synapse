use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use async_trait::async_trait;
use tracing;

use crate::agent;
use crate::channels::formatter;
use crate::channels::handler::AgentSession;
use crate::config::bots::resolve_secret;
use crate::config::SynapseConfig;
use crate::gateway::messages::{ChannelInfo, ChatInfo, InboundMessage, SenderInfo};
use synaptic::core::{
    ChannelAdapter, ChannelCap, ChannelContext, ChannelHealth, ChannelManifest, ChannelStatus,
    HealthStatus, MessageEnvelope as CoreMessageEnvelope, Outbound, RunContext,
};

/// Run the iMessage bot adapter using the BlueBubbles REST API bridge.
///
/// Polls `GET /api/v1/message?limit=10&offset=0&after=<timestamp>&password=<pw>`
/// for incoming messages and replies via `POST /api/v1/message/text`.
pub async fn run(
    config: &SynapseConfig,
    model_override: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let imessage_configs: Vec<crate::config::IMessageBotConfig> = config.channel_configs("imessage");
    let imessage_config = imessage_configs
        .first()
        .ok_or("missing [[channels.imessage]] section in config")?;

    let password = resolve_secret(
        imessage_config.password.as_deref(),
        imessage_config.password_env.as_deref(),
        "iMessage password",
    )
    .map_err(|e| format!("{}", e))?;

    let model = agent::build_model(config, model_override)?;
    let config_arc = Arc::new(config.clone());
    let allowlist = imessage_config.allowlist.clone();
    let agent_session = Arc::new(AgentSession::new(model, config_arc, true));

    let api_url = imessage_config.api_url.trim_end_matches('/').to_string();

    if !allowlist.is_empty() {
        tracing::info!(
            channel = "imessage",
            users = allowlist.allowed_users.len(),
            channels = allowlist.allowed_channels.len(),
            "allowlist active"
        );
    }

    tracing::info!(channel = "imessage", "adapter started");
    tracing::info!(channel = "imessage", api_url = %api_url, "polling started");

    let client = reqwest::Client::new();

    // Track the last poll timestamp in milliseconds (Unix epoch).
    // BlueBubbles uses millisecond timestamps for the `after` parameter.
    let mut last_timestamp_ms: u64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64;

    loop {
        let poll_url = format!(
            "{}/api/v1/message?limit=10&offset=0&after={}&password={}",
            api_url,
            last_timestamp_ms,
            urlencoding::encode(&password)
        );

        let resp = match client.get(&poll_url).send().await {
            Ok(r) => r,
            Err(e) => {
                tracing::warn!(channel = "imessage", error = %e, "polling error");
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        let body: serde_json::Value = match resp.json().await {
            Ok(v) => v,
            Err(e) => {
                tracing::warn!(channel = "imessage", error = %e, "response parse error");
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        // BlueBubbles wraps results in { "status": "ok", "data": [...] }
        let messages = match body.get("data").and_then(|d| d.as_array()) {
            Some(arr) if !arr.is_empty() => arr.clone(),
            _ => {
                tokio::time::sleep(Duration::from_secs(2)).await;
                continue;
            }
        };

        // Advance the timestamp so the next poll only returns newer messages.
        // Use the current wall-clock time to avoid re-processing the same batch.
        last_timestamp_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;

        for message in messages {
            // Only process incoming text messages (not sent by the bot itself)
            let is_from_me = message
                .get("isFromMe")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            if is_from_me {
                continue;
            }

            // Extract the message text
            let text = match message.get("text").and_then(|v| v.as_str()) {
                Some(t) if !t.is_empty() => t.to_string(),
                _ => continue,
            };

            // The chat GUID identifies the conversation to reply to
            let chat_guid = match message
                .get("chats")
                .and_then(|c| c.as_array())
                .and_then(|arr| arr.first())
                .and_then(|chat| chat.get("guid"))
                .and_then(|v| v.as_str())
            {
                Some(g) => g.to_string(),
                None => continue,
            };

            // The sender handle is used as the session/user identifier
            let sender = match message
                .get("handle")
                .and_then(|h| h.get("address"))
                .and_then(|v| v.as_str())
            {
                Some(s) if !s.is_empty() => s.to_string(),
                _ => chat_guid.clone(),
            };

            // Allowlist check: sender is the user, chat_guid acts as the channel
            if !allowlist.is_allowed(Some(&sender), Some(&chat_guid)) {
                continue;
            }

            // Process message in background
            let session = agent_session.clone();
            let http = client.clone();
            let send_url = format!("{}/api/v1/message/text", api_url);
            let pw = password.clone();
            let reply_chat_guid = chat_guid.clone();
            let session_key = sender.clone();

            // iMessage group chats have GUIDs starting with "iMessage;+;" while DMs are "iMessage;-;"
            let is_group = chat_guid.contains(";+;");

            tokio::spawn(async move {
                let channel_info = ChannelInfo {
                    platform: "imessage".into(),
                    native_channel_id: Some(reply_chat_guid.clone()),
                    ..Default::default()
                };
                let sender_info = SenderInfo {
                    id: Some(sender.clone()),
                    ..Default::default()
                };
                let chat_info = ChatInfo {
                    chat_type: if is_group { "group" } else { "direct" }.to_string(),
                    ..Default::default()
                };
                let mut msg = InboundMessage::channel(
                    session_key.clone(),
                    text.clone(),
                    channel_info,
                    sender_info,
                    chat_info,
                );
                msg.finalize();
                match session.handle_message(msg, RunContext::default()).await {
                    Ok(reply) => {
                        let chunks =
                            formatter::format_for_channel(&reply.content, "imessage", 10000);
                        for chunk in chunks {
                            let body = serde_json::json!({
                                "chatGuid": reply_chat_guid,
                                "message": chunk,
                                "password": pw,
                            });
                            if let Err(e) = http.post(&send_url).json(&body).send().await {
                                tracing::error!(channel = "imessage", error = %e, "send error");
                            }
                        }
                    }
                    Err(e) => {
                        tracing::error!(channel = "imessage", error = %e, "handler error");
                    }
                }
            });
        }
    }
}

// ---------------------------------------------------------------------------
// ChannelAdapter / Outbound / ChannelHealth trait implementations
// ---------------------------------------------------------------------------

/// Status constants used by [`IMessageAdapter`].
const STATUS_DISCONNECTED: u8 = 0;
const STATUS_CONNECTED: u8 = 1;
const STATUS_ERROR: u8 = 2;

/// Channel adapter facade for the iMessage (BlueBubbles) bridge.
#[allow(dead_code)]
pub struct IMessageAdapter {
    api_url: String,
    client: reqwest::Client,
    /// Atomic status: 0 = Disconnected, 1 = Connected, 2 = Error.
    status: AtomicU8,
}

#[allow(dead_code)]
impl IMessageAdapter {
    pub fn new(api_url: &str) -> Self {
        Self {
            api_url: api_url.trim_end_matches('/').to_string(),
            client: reqwest::Client::new(),
            status: AtomicU8::new(STATUS_DISCONNECTED),
        }
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelAdapter for IMessageAdapter {
    fn manifest(&self) -> ChannelManifest {
        ChannelManifest {
            id: "imessage".to_string(),
            name: "iMessage".to_string(),
            capabilities: vec![
                ChannelCap::Inbound,
                ChannelCap::Outbound,
                ChannelCap::Groups,
                ChannelCap::Health,
            ],
            message_limit: Some(20000),
            supports_streaming: false,
            supports_threads: false,
            supports_reactions: false,
        }
    }

    async fn start(&self, _ctx: ChannelContext) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_CONNECTED, Ordering::SeqCst);
        tracing::info!(channel = "imessage", "IMessageAdapter started");
        Ok(())
    }

    async fn stop(&self) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_DISCONNECTED, Ordering::SeqCst);
        tracing::info!(channel = "imessage", "IMessageAdapter stopped");
        Ok(())
    }

    fn status(&self) -> ChannelStatus {
        match self.status.load(Ordering::SeqCst) {
            STATUS_CONNECTED => ChannelStatus::Connected,
            STATUS_ERROR => ChannelStatus::Error("adapter error".to_string()),
            _ => ChannelStatus::Disconnected,
        }
    }
}

#[allow(dead_code)]
#[async_trait]
impl Outbound for IMessageAdapter {
    async fn send(
        &self,
        envelope: &CoreMessageEnvelope,
    ) -> Result<(), synaptic::core::SynapticError> {
        tracing::info!(
            channel = "imessage",
            to = %envelope.channel_id,
            "IMessageAdapter::send (placeholder)",
        );
        Ok(())
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelHealth for IMessageAdapter {
    async fn health_check(&self) -> HealthStatus {
        let url = format!("{}/api/v1/server/info", self.api_url);
        match self.client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                self.status.store(STATUS_CONNECTED, Ordering::SeqCst);
                HealthStatus::Healthy
            }
            Ok(resp) => {
                let msg = format!("BlueBubbles health returned HTTP {}", resp.status());
                self.status.store(STATUS_ERROR, Ordering::SeqCst);
                HealthStatus::Unhealthy(msg)
            }
            Err(e) => {
                self.status.store(STATUS_ERROR, Ordering::SeqCst);
                HealthStatus::Unhealthy(e.to_string())
            }
        }
    }
}
