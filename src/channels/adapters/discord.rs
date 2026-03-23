//! Discord bot adapter.

use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use colored::Colorize;
use futures::{SinkExt, StreamExt};
use serde_json::json;
use tokio_tungstenite::tungstenite::Message as WsMsg;
use tracing;

use synaptic::core::{
    ChannelAdapter, ChannelCap, ChannelContext, ChannelHealth, ChannelManifest, ChannelStatus,
    HealthStatus, MessageEnvelope as CoreMessageEnvelope, Outbound, RunContext,
};
use synaptic::DeliveryContext;

use crate::agent;
use crate::channels::formatter;
use crate::channels::handler::AgentSession;
use crate::channels::reactions;
use crate::config::bots::resolve_secret;
use crate::config::SynapseConfig;
use crate::gateway::messages::sender::{ChannelSender, SendResult};
use crate::gateway::messages::{Attachment, ChannelInfo, ChatInfo, InboundMessage, SenderInfo};
use crate::gateway::presence::now_ms;
use synaptic::logging;

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
    ) -> crate::error::Result<SendResult> {
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
pub async fn run(config: &SynapseConfig, model_override: Option<&str>) -> crate::error::Result<()> {
    let discord_configs: Vec<crate::config::DiscordBotConfig> = config.channel_configs("discord");
    let discord_config = discord_configs
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
                    let channel_info = ChannelInfo {
                        platform: "discord".into(),
                        native_channel_id: Some(channel_id.clone()),
                        guild_id: guild_id.clone(),
                        ..Default::default()
                    };
                    let sender_info = SenderInfo {
                        id: Some(sender_id.clone()),
                        ..Default::default()
                    };
                    let chat_info = ChatInfo {
                        chat_type: if is_dm { "direct" } else { "group" }.to_string(),
                        ..Default::default()
                    };
                    let mut msg = InboundMessage::channel(
                        channel_id.clone(),
                        content,
                        channel_info,
                        sender_info,
                        chat_info,
                    );
                    msg.attachments = attachments;
                    msg.finalize();

                    match session.handle_message(msg, RunContext::default()).await {
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

// ---------------------------------------------------------------------------
// ChannelAdapter / Outbound / ChannelHealth trait implementations
// ---------------------------------------------------------------------------

/// Status constants used by [`DiscordAdapter`].
const STATUS_DISCONNECTED: u8 = 0;
const STATUS_CONNECTED: u8 = 1;
const STATUS_ERROR: u8 = 2;

/// Channel adapter facade for the Discord bot.
///
/// Wraps an HTTP client and bot token so the generic channel infrastructure
/// can call `start`, `stop`, `send`, and `health_check` without knowing
/// anything about the Discord-specific WebSocket gateway loop.
#[allow(dead_code)]
pub struct DiscordAdapter {
    client: reqwest::Client,
    token: String,
    /// Atomic status: 0 = Disconnected, 1 = Connected, 2 = Error.
    status: AtomicU8,
}

#[allow(dead_code)]
impl DiscordAdapter {
    pub fn new(bot_token: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            token: bot_token.to_string(),
            status: AtomicU8::new(STATUS_DISCONNECTED),
        }
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelAdapter for DiscordAdapter {
    fn manifest(&self) -> ChannelManifest {
        ChannelManifest {
            id: "discord".to_string(),
            name: "Discord".to_string(),
            capabilities: vec![
                ChannelCap::Inbound,
                ChannelCap::Outbound,
                ChannelCap::Groups,
                ChannelCap::Reactions,
                ChannelCap::Health,
            ],
            message_limit: Some(2000),
            supports_streaming: false,
            supports_threads: true,
            supports_reactions: true,
        }
    }

    async fn start(&self, _ctx: ChannelContext) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_CONNECTED, Ordering::SeqCst);
        tracing::info!(channel = "discord", "DiscordAdapter started");
        Ok(())
    }

    async fn stop(&self) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_DISCONNECTED, Ordering::SeqCst);
        tracing::info!(channel = "discord", "DiscordAdapter stopped");
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
impl Outbound for DiscordAdapter {
    async fn send(
        &self,
        envelope: &CoreMessageEnvelope,
    ) -> Result<(), synaptic::core::SynapticError> {
        // Route to the channel encoded in thread_id ("channel:<id>") or channel_id.
        let raw_channel = envelope
            .thread_id
            .as_deref()
            .unwrap_or(envelope.channel_id.as_str());
        let channel_id = raw_channel.strip_prefix("channel:").unwrap_or(raw_channel);

        let chunks = formatter::format_for_channel(&envelope.content, "discord", 2000);
        for chunk in chunks {
            self.client
                .post(format!(
                    "https://discord.com/api/v10/channels/{}/messages",
                    channel_id
                ))
                .header("Authorization", format!("Bot {}", self.token))
                .json(&json!({"content": chunk}))
                .send()
                .await
                .map_err(|e| synaptic::core::SynapticError::Tool(e.to_string()))?;
        }
        Ok(())
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelHealth for DiscordAdapter {
    async fn health_check(&self) -> HealthStatus {
        let url = "https://discord.com/api/v10/users/@me";
        match self
            .client
            .get(url)
            .header("Authorization", format!("Bot {}", self.token))
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                self.status.store(STATUS_CONNECTED, Ordering::SeqCst);
                HealthStatus::Healthy
            }
            Ok(resp) => {
                let msg = format!("GET /users/@me returned HTTP {}", resp.status());
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
