use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use futures::{SinkExt, StreamExt};
use tokio_tungstenite::tungstenite::Message as WsMsg;

use tracing;

use synaptic::core::{
    ChannelAdapter, ChannelCap, ChannelContext, ChannelHealth, ChannelManifest, ChannelStatus,
    HealthStatus, MessageEnvelope as CoreMessageEnvelope, Outbound,
};

use synaptic::DeliveryContext;

use crate::agent;
use crate::channels::formatter;
use crate::channels::handler::AgentSession;
use crate::config::bots::resolve_secret;
use crate::config::{BotAllowlist, SynapseConfig};
use crate::gateway::messages::MessageEnvelope;

/// Run the Mattermost bot adapter using WebSocket events.
pub async fn run(
    config: &SynapseConfig,
    model_override: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mm_config = config
        .mattermost
        .first()
        .ok_or("missing [[mattermost]] section in config")?;

    let token = resolve_secret(
        mm_config.token.as_deref(),
        mm_config.token_env.as_deref(),
        "Mattermost token",
    )
    .map_err(|e| format!("{}", e))?;

    // Get the bot's own user ID so we can skip our own messages
    let bot_user_id = get_bot_user_id(&mm_config.url, &token).await?;

    let model = agent::build_model(config, model_override)?;
    let config_arc = Arc::new(config.clone());
    let allowlist = mm_config.allowlist.clone();
    let agent_session = Arc::new(AgentSession::new(model, config_arc, true));

    if !allowlist.is_empty() {
        tracing::info!(
            channel = "mattermost",
            users = allowlist.allowed_users.len(),
            channels = allowlist.allowed_channels.len(),
            "allowlist active"
        );
    }

    tracing::info!(channel = "mattermost", "adapter started");

    loop {
        match run_ws(
            &mm_config.url,
            &token,
            &bot_user_id,
            agent_session.clone(),
            &allowlist,
        )
        .await
        {
            Ok(()) => break,
            Err(e) => {
                tracing::warn!(channel = "mattermost", error = %e, "connection error, reconnecting");
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
        }
    }

    Ok(())
}

/// Fetch the bot's own user ID via REST API.
async fn get_bot_user_id(url: &str, token: &str) -> Result<String, Box<dyn std::error::Error>> {
    let client = reqwest::Client::new();
    let resp = client
        .get(format!("{}/api/v4/users/me", url))
        .bearer_auth(token)
        .send()
        .await?;

    let body: serde_json::Value = resp.json().await?;
    body.get("id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .ok_or_else(|| "failed to get bot user ID from /api/v4/users/me".into())
}

/// Connect via WebSocket, authenticate, and process events.
async fn run_ws(
    url: &str,
    token: &str,
    bot_user_id: &str,
    agent_session: Arc<AgentSession>,
    allowlist: &BotAllowlist,
) -> Result<(), Box<dyn std::error::Error>> {
    // Build WSS URL: replace http(s) with ws(s)
    let ws_url = if url.starts_with("https://") {
        format!("wss://{}/api/v4/websocket", &url["https://".len()..])
    } else if url.starts_with("http://") {
        format!("ws://{}/api/v4/websocket", &url["http://".len()..])
    } else {
        format!("wss://{}/api/v4/websocket", url)
    };

    let (ws_stream, _) = tokio_tungstenite::connect_async(&ws_url).await?;
    let (mut write, mut read) = ws_stream.split();

    // Authenticate
    let auth_msg = serde_json::json!({
        "seq": 1,
        "action": "authentication_challenge",
        "data": { "token": token }
    });
    write.send(WsMsg::Text(auth_msg.to_string().into())).await?;

    tracing::info!(channel = "mattermost", "websocket connected");

    // Listen for events
    while let Some(msg) = read.next().await {
        let msg = msg?;
        let WsMsg::Text(text) = msg else { continue };

        let payload: serde_json::Value = match serde_json::from_str(&text) {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Only handle "posted" events
        let event = payload.get("event").and_then(|v| v.as_str()).unwrap_or("");
        if event != "posted" {
            continue;
        }

        // The post data is a JSON string inside data.post
        let post_str = match payload
            .get("data")
            .and_then(|d| d.get("post"))
            .and_then(|p| p.as_str())
        {
            Some(s) => s,
            None => continue,
        };

        let post: serde_json::Value = match serde_json::from_str(post_str) {
            Ok(v) => v,
            Err(_) => continue,
        };

        let user_id = post.get("user_id").and_then(|v| v.as_str()).unwrap_or("");
        let channel_id = post
            .get("channel_id")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();
        let message = post
            .get("message")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string();

        // Skip bot's own messages
        if user_id == bot_user_id {
            continue;
        }

        if message.is_empty() || channel_id.is_empty() {
            continue;
        }

        // Allowlist check
        if !allowlist.is_allowed(Some(user_id), Some(&channel_id)) {
            continue;
        }

        // Process in background
        let session = agent_session.clone();
        let api_url = url.to_string();
        let api_token = token.to_string();
        let sender_id = user_id.to_string();
        tokio::spawn(async move {
            let mut envelope = MessageEnvelope::channel(
                channel_id.clone(),
                message.clone(),
                DeliveryContext {
                    channel: "mattermost".into(),
                    to: Some(format!("channel:{}", channel_id)),
                    account_id: None,
                    thread_id: None,
                    meta: None,
                },
            );
            envelope.sender_id = Some(sender_id);
            envelope.routing.peer_kind = Some(crate::config::PeerKind::Channel);
            envelope.routing.peer_id = Some(channel_id.clone());
            match session.handle_message(envelope).await {
                Ok(reply) => {
                    let chunks = formatter::format_for_channel(&reply.content, "mattermost", 16383);
                    let client = reqwest::Client::new();
                    for chunk in chunks {
                        let _ = client
                            .post(format!("{}/api/v4/posts", api_url))
                            .bearer_auth(&api_token)
                            .json(&serde_json::json!({
                                "channel_id": channel_id,
                                "message": chunk,
                            }))
                            .send()
                            .await;
                    }
                }
                Err(e) => {
                    tracing::error!(channel = "mattermost", error = %e, "message handler error");
                }
            }
        });
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// ChannelAdapter / Outbound / ChannelHealth trait implementations
// ---------------------------------------------------------------------------

const STATUS_DISCONNECTED: u8 = 0;
const STATUS_CONNECTED: u8 = 1;
const STATUS_ERROR: u8 = 2;

/// Channel adapter facade for the Mattermost bot.
#[allow(dead_code)]
pub struct MattermostAdapter {
    client: reqwest::Client,
    /// Atomic status: 0 = Disconnected, 1 = Connected, 2 = Error.
    status: AtomicU8,
}

#[allow(dead_code)]
impl MattermostAdapter {
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
            status: AtomicU8::new(STATUS_DISCONNECTED),
        }
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelAdapter for MattermostAdapter {
    fn manifest(&self) -> ChannelManifest {
        ChannelManifest {
            id: "mattermost".to_string(),
            name: "Mattermost".to_string(),
            capabilities: vec![
                ChannelCap::Inbound,
                ChannelCap::Outbound,
                ChannelCap::Groups,
                ChannelCap::Health,
            ],
            message_limit: Some(16383),
            supports_streaming: false,
            supports_threads: false,
            supports_reactions: false,
        }
    }

    async fn start(&self, _ctx: ChannelContext) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_CONNECTED, Ordering::SeqCst);
        tracing::info!(channel = "mattermost", "MattermostAdapter started");
        Ok(())
    }

    async fn stop(&self) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_DISCONNECTED, Ordering::SeqCst);
        tracing::info!(channel = "mattermost", "MattermostAdapter stopped");
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
impl Outbound for MattermostAdapter {
    async fn send(
        &self,
        envelope: &CoreMessageEnvelope,
    ) -> Result<(), synaptic::core::SynapticError> {
        tracing::info!(
            channel = "mattermost",
            content_len = envelope.content.len(),
            "MattermostAdapter::send (placeholder)"
        );
        Ok(())
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelHealth for MattermostAdapter {
    async fn health_check(&self) -> HealthStatus {
        match self.status.load(Ordering::SeqCst) {
            STATUS_CONNECTED => HealthStatus::Healthy,
            STATUS_ERROR => HealthStatus::Unhealthy("adapter error".to_string()),
            _ => HealthStatus::Unhealthy("disconnected".to_string()),
        }
    }
}
