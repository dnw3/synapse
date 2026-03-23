use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures::{SinkExt, StreamExt};
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
use crate::config::{BotAllowlist, SynapseConfig};
use crate::gateway::messages::sender::{ChannelSender, SendResult};
use crate::gateway::messages::{ChannelInfo, ChatInfo, InboundMessage, SenderInfo};
use crate::gateway::presence::now_ms;

// ---------------------------------------------------------------------------
// ChannelSender implementation
// ---------------------------------------------------------------------------

/// Outbound sender for the Slack channel.
#[allow(dead_code)]
pub struct SlackSender {
    /// Bot OAuth token used for `chat.postMessage`.
    pub bot_token: String,
}

#[async_trait]
impl ChannelSender for SlackSender {
    fn channel_id(&self) -> &str {
        "slack"
    }

    async fn send(
        &self,
        target: &DeliveryContext,
        content: &str,
        _meta: Option<&serde_json::Value>,
    ) -> crate::error::Result<SendResult> {
        let channel = target
            .to
            .as_deref()
            .and_then(|s| s.strip_prefix("channel:"))
            .ok_or("missing or invalid channel in delivery target (expected 'channel:<id>')")?;

        let client = reqwest::Client::new();
        let chunks = formatter::format_for_channel(content, "slack", 4000);
        let mut last_ts: Option<String> = None;
        for chunk in chunks {
            let resp: serde_json::Value = client
                .post("https://slack.com/api/chat.postMessage")
                .bearer_auth(&self.bot_token)
                .json(&serde_json::json!({
                    "channel": channel,
                    "text": chunk,
                }))
                .send()
                .await?
                .json()
                .await?;
            if let Some(ts) = resp.get("ts").and_then(|v| v.as_str()) {
                last_ts = Some(ts.to_string());
            }
        }

        Ok(SendResult {
            message_id: last_ts,
            delivered_at_ms: now_ms(),
        })
    }
}

/// Run the Slack bot adapter using Socket Mode.
pub async fn run(config: &SynapseConfig, model_override: Option<&str>) -> crate::error::Result<()> {
    let slack_configs: Vec<crate::config::SlackBotConfig> = config.channel_configs("slack");
    let slack_config = slack_configs
        .first()
        .ok_or("missing [[channels.slack]] section in config")?;

    let app_token = resolve_secret(
        slack_config.app_token.as_deref(),
        slack_config.app_token_env.as_deref(),
        "Slack app token",
    )
    .map_err(|e| e.to_string())?;
    let bot_token = resolve_secret(
        slack_config.bot_token.as_deref(),
        slack_config.bot_token_env.as_deref(),
        "Slack bot token",
    )
    .map_err(|e| e.to_string())?;

    let model = agent::build_model(config, model_override)?;
    let config_arc = Arc::new(config.clone());
    let allowlist = slack_config.allowlist.clone();
    let agent_session = Arc::new(AgentSession::new(model, config_arc, true).with_channel("slack"));

    if !allowlist.is_empty() {
        tracing::info!(
            channel = "slack",
            users = allowlist.allowed_users.len(),
            channels = allowlist.allowed_channels.len(),
            "allowlist active"
        );
    }

    tracing::info!(channel = "slack", "adapter started");

    loop {
        let err_msg = match run_socket_mode(
            &app_token,
            &bot_token,
            agent_session.clone(),
            &allowlist,
        )
        .await
        {
            Ok(()) => break,
            Err(e) => e.to_string(),
        };
        tracing::warn!(channel = "slack", error = %err_msg, "connection error, reconnecting");
        tokio::time::sleep(Duration::from_secs(5)).await;
    }

    Ok(())
}

/// Send a typing indicator to a Slack channel.
///
/// Note: Slack's Socket Mode does not expose a typing API for bots. The
/// `chat.postMessage` API only sends messages, not ephemeral typing indicators.
/// Alternative approach: post a temporary "Thinking..." message and update it
/// with the real response via `chat.update`. This is left as a future enhancement
/// since it requires tracking the temporary message ts.
async fn send_typing(_bot_token: &str, _channel: &str) {
    // Slack Socket Mode has no typing API for bots — intentional no-op.
}

// ---------------------------------------------------------------------------
// ChannelAdapter / Outbound / ChannelHealth trait implementations
// ---------------------------------------------------------------------------

/// Status constants used by [`SlackAdapter`].
const STATUS_DISCONNECTED: u8 = 0;
const STATUS_CONNECTED: u8 = 1;
const STATUS_ERROR: u8 = 2;

/// Channel adapter facade for the Slack bot.
///
/// Wraps an HTTP client and bot token so the generic channel infrastructure
/// can call `start`, `stop`, `send`, and `health_check` without knowing
/// anything about the Slack-specific Socket Mode loop.
#[allow(dead_code)]
pub struct SlackAdapter {
    client: reqwest::Client,
    bot_token: String,
    /// Atomic status: 0 = Disconnected, 1 = Connected, 2 = Error.
    status: AtomicU8,
}

#[allow(dead_code)]
impl SlackAdapter {
    pub fn new(bot_token: &str) -> Self {
        Self {
            client: reqwest::Client::new(),
            bot_token: bot_token.to_string(),
            status: AtomicU8::new(STATUS_DISCONNECTED),
        }
    }
}

#[allow(unused)]
#[async_trait]
impl ChannelAdapter for SlackAdapter {
    fn manifest(&self) -> ChannelManifest {
        ChannelManifest {
            id: "slack".to_string(),
            name: "Slack".to_string(),
            capabilities: vec![
                ChannelCap::Inbound,
                ChannelCap::Outbound,
                ChannelCap::Groups,
                ChannelCap::Reactions,
                ChannelCap::Threading,
                ChannelCap::Mentions,
                ChannelCap::Health,
            ],
            message_limit: Some(40000),
            supports_streaming: false,
            supports_threads: true,
            supports_reactions: true,
        }
    }

    async fn start(&self, _ctx: ChannelContext) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_CONNECTED, Ordering::SeqCst);
        tracing::info!(channel = "slack", "SlackAdapter started");
        Ok(())
    }

    async fn stop(&self) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_DISCONNECTED, Ordering::SeqCst);
        tracing::info!(channel = "slack", "SlackAdapter stopped");
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

#[allow(unused)]
#[async_trait]
impl Outbound for SlackAdapter {
    async fn send(
        &self,
        envelope: &CoreMessageEnvelope,
    ) -> Result<(), synaptic::core::SynapticError> {
        // Route to the channel encoded in thread_id ("channel:<id>") or channel_id.
        let raw_channel = envelope
            .thread_id
            .as_deref()
            .unwrap_or(envelope.channel_id.as_str());
        let channel = raw_channel.strip_prefix("channel:").unwrap_or(raw_channel);

        let chunks = formatter::format_for_channel(&envelope.content, "slack", 4000);
        for chunk in chunks {
            self.client
                .post("https://slack.com/api/chat.postMessage")
                .bearer_auth(&self.bot_token)
                .json(&serde_json::json!({
                    "channel": channel,
                    "text": chunk,
                }))
                .send()
                .await
                .map_err(|e| synaptic::core::SynapticError::Tool(e.to_string()))?;
        }
        Ok(())
    }
}

#[allow(unused)]
#[async_trait]
impl ChannelHealth for SlackAdapter {
    async fn health_check(&self) -> HealthStatus {
        // Use auth.test as a lightweight liveness probe.
        match self
            .client
            .post("https://slack.com/api/auth.test")
            .bearer_auth(&self.bot_token)
            .send()
            .await
        {
            Ok(resp) if resp.status().is_success() => {
                match resp.json::<serde_json::Value>().await {
                    Ok(body) if body.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) => {
                        self.status.store(STATUS_CONNECTED, Ordering::SeqCst);
                        HealthStatus::Healthy
                    }
                    Ok(body) => {
                        let msg = body
                            .get("error")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown error")
                            .to_string();
                        self.status.store(STATUS_ERROR, Ordering::SeqCst);
                        HealthStatus::Unhealthy(msg)
                    }
                    Err(e) => {
                        self.status.store(STATUS_ERROR, Ordering::SeqCst);
                        HealthStatus::Unhealthy(e.to_string())
                    }
                }
            }
            Ok(resp) => {
                let msg = format!("auth.test returned HTTP {}", resp.status());
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

async fn run_socket_mode(
    app_token: &str,
    bot_token: &str,
    agent_session: Arc<AgentSession>,
    allowlist: &BotAllowlist,
) -> crate::error::Result<()> {
    // Step 1: Open a WebSocket connection via apps.connections.open
    let client = reqwest::Client::new();
    let resp = client
        .post("https://slack.com/api/apps.connections.open")
        .bearer_auth(app_token)
        .send()
        .await?;

    let body: serde_json::Value = resp.json().await?;
    if !body.get("ok").and_then(|v| v.as_bool()).unwrap_or(false) {
        return Err(format!("apps.connections.open failed: {}", body).into());
    }

    let ws_url = body
        .get("url")
        .and_then(|v| v.as_str())
        .ok_or("no WebSocket URL returned")?;

    // Step 2: Connect WebSocket
    let (ws_stream, _) = tokio_tungstenite::connect_async(ws_url).await?;
    let (mut write, mut read) = ws_stream.split();

    tracing::info!(channel = "slack", "socket mode connected");

    // Step 3: Handle events
    while let Some(msg) = read.next().await {
        let msg = msg?;
        let WsMsg::Text(text) = msg else { continue };

        let payload: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Acknowledge the envelope
        if let Some(envelope_id) = payload.get("envelope_id").and_then(|v| v.as_str()) {
            let ack = serde_json::json!({"envelope_id": envelope_id});
            write.send(WsMsg::Text(ack.to_string().into())).await.ok();
        }

        // Check event type
        let event_type = payload.get("type").and_then(|v| v.as_str()).unwrap_or("");

        if event_type != "events_api" {
            continue;
        }

        // Extract message event
        let event = match payload.get("payload").and_then(|p| p.get("event")) {
            Some(e) => e,
            None => continue,
        };

        let msg_type = event.get("type").and_then(|v| v.as_str()).unwrap_or("");
        if msg_type != "message" {
            continue;
        }

        // Skip bot messages (prevent echo)
        if event.get("bot_id").is_some() {
            continue;
        }

        let text = event
            .get("text")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let channel = event
            .get("channel")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let user_id = event.get("user").and_then(|v| v.as_str()).unwrap_or("");
        let channel_type = event
            .get("channel_type")
            .and_then(|v| v.as_str())
            .unwrap_or("");

        let ts = event
            .get("ts")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        if text.is_empty() || channel.is_empty() {
            continue;
        }

        // Allowlist check
        if !allowlist.is_allowed(Some(user_id), Some(&channel)) {
            continue;
        }

        let is_dm = channel_type == "im";
        let team_id = payload
            .get("payload")
            .and_then(|p| p.get("team_id"))
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());
        let sender_id = user_id.to_string();

        // Process in background
        let session = agent_session.clone();
        let bot_token = bot_token.to_string();
        tokio::spawn(async move {
            // React with eyes to indicate processing
            reactions::slack_react(&bot_token, &channel, &ts, "eyes").await;

            // Send typing indicator
            send_typing(&bot_token, &channel).await;

            let channel_info = ChannelInfo {
                platform: "slack".into(),
                native_channel_id: Some(channel.clone()),
                team_id: team_id.clone(),
                ..Default::default()
            };
            let sender_info = SenderInfo {
                id: Some(sender_id.clone()),
                ..Default::default()
            };
            let chat_info = ChatInfo {
                chat_type: if is_dm { "direct" } else { "channel" }.to_string(),
                ..Default::default()
            };
            let mut msg = InboundMessage::channel(
                channel.clone(),
                text,
                channel_info,
                sender_info,
                chat_info,
            );
            msg.thread.thread_id = Some(ts.clone());
            msg.finalize();

            match session.handle_message(msg, RunContext::default()).await {
                Ok(reply) => {
                    // Split long replies into chunks
                    let chunks = formatter::format_for_channel(&reply.content, "slack", 4000);
                    let client = reqwest::Client::new();
                    for chunk in chunks {
                        let _ = client
                            .post("https://slack.com/api/chat.postMessage")
                            .bearer_auth(&bot_token)
                            .json(&serde_json::json!({
                                "channel": channel,
                                "text": chunk,
                            }))
                            .send()
                            .await;
                    }
                    // React with checkmark on success
                    reactions::slack_react(&bot_token, &channel, &ts, "white_check_mark").await;
                }
                Err(e) => {
                    tracing::error!(channel = "slack", error = %e, "message handler error");
                }
            }
        });
    }

    Ok(())
}
