use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use serde::Deserialize;

use crate::agent;
use crate::channels::handler::AgentSession;
use crate::config::bots::resolve_secret;
use crate::config::{SynapseConfig, ZaloBotConfig};
use crate::gateway::messages::{ChannelInfo, ChatInfo, InboundMessage, SenderInfo};
use synaptic::core::{
    ChannelAdapter, ChannelCap, ChannelContext, ChannelHealth, ChannelManifest, ChannelStatus,
    HealthStatus, MessageEnvelope as CoreMessageEnvelope, Outbound,
};

#[derive(Clone)]
struct AppState {
    agent_session: Arc<AgentSession>,
    allowlist: crate::config::BotAllowlist,
    access_token: String,
}

#[derive(Deserialize)]
struct ZaloWebhook {
    #[serde(default)]
    event_name: Option<String>,
    #[serde(default)]
    sender: Option<ZaloSender>,
    #[serde(default)]
    message: Option<ZaloMessage>,
}

#[derive(Deserialize)]
struct ZaloSender {
    #[serde(default)]
    id: String,
}

#[derive(Deserialize)]
struct ZaloMessage {
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    msg_id: Option<String>,
}

/// Run the Zalo bot adapter (webhook HTTP mode).
pub async fn run(
    config: &SynapseConfig,
    model_override: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let zalo_config = config
        .zalo
        .first()
        .ok_or("missing [[zalo]] section in config")?;

    let access_token = resolve_secret(
        zalo_config.access_token.as_deref(),
        zalo_config.access_token_env.as_deref(),
        "Zalo access token",
    )
    .map_err(|e| format!("{}", e))?;

    let model = agent::build_model(config, model_override)?;
    let config_arc = Arc::new(config.clone());
    let agent_session = Arc::new(AgentSession::new(model, config_arc, true));

    let port = zalo_config.port.unwrap_or(8092);

    let state = AppState {
        agent_session,
        allowlist: zalo_config.allowlist.clone(),
        access_token,
    };

    let app = Router::new()
        .route("/webhook", post(handle_webhook))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    tracing::info!(channel = "zalo", addr = %addr, "adapter started");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn handle_webhook(
    State(state): State<AppState>,
    Json(payload): Json<ZaloWebhook>,
) -> impl IntoResponse {
    // Only handle "user_send_text" events
    if payload.event_name.as_deref() != Some("user_send_text") {
        return StatusCode::OK;
    }

    let sender_id = match payload.sender {
        Some(ref s) if !s.id.is_empty() => s.id.clone(),
        _ => return StatusCode::OK,
    };

    let text = match payload.message.and_then(|m| m.text) {
        Some(t) if !t.is_empty() => t,
        _ => return StatusCode::OK,
    };

    if !state.allowlist.is_allowed(Some(&sender_id), None) {
        return StatusCode::FORBIDDEN;
    }

    let session = state.agent_session.clone();
    let access_token = state.access_token.clone();

    tokio::spawn(async move {
        let channel_info = ChannelInfo {
            platform: "zalo".into(),
            ..Default::default()
        };
        let sender_info = SenderInfo {
            id: Some(sender_id.clone()),
            ..Default::default()
        };
        let chat_info = ChatInfo {
            chat_type: "direct".into(),
            ..Default::default()
        };
        let mut msg = InboundMessage::channel(
            sender_id.clone(),
            text.clone(),
            channel_info,
            sender_info,
            chat_info,
        );
        msg.finalize();
        match session.handle_message(msg).await {
            Ok(reply) => {
                // Send reply via Zalo OA API
                let client = reqwest::Client::new();
                let _ = client
                    .post("https://openapi.zalo.me/v3.0/oa/message/cs")
                    .header("access_token", &access_token)
                    .json(&serde_json::json!({
                        "recipient": {"user_id": sender_id},
                        "message": {"text": reply.content}
                    }))
                    .send()
                    .await;
            }
            Err(e) => {
                tracing::error!(channel = "zalo", error = %e, "agent error");
            }
        }
    });

    StatusCode::OK
}

// ---------------------------------------------------------------------------
// ChannelAdapter / Outbound / ChannelHealth trait implementations
// ---------------------------------------------------------------------------

/// Status constants used by [`ZaloAdapter`].
const STATUS_DISCONNECTED: u8 = 0;
const STATUS_CONNECTED: u8 = 1;
const STATUS_ERROR: u8 = 2;

/// Channel adapter facade for the Zalo OA bot.
#[allow(dead_code)]
pub struct ZaloAdapter {
    client: reqwest::Client,
    access_token: String,
    /// Atomic status: 0 = Disconnected, 1 = Connected, 2 = Error.
    status: AtomicU8,
}

#[allow(dead_code)]
impl ZaloAdapter {
    pub fn new(access_token: String) -> Self {
        Self {
            client: reqwest::Client::new(),
            access_token,
            status: AtomicU8::new(STATUS_DISCONNECTED),
        }
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelAdapter for ZaloAdapter {
    fn manifest(&self) -> ChannelManifest {
        ChannelManifest {
            id: "zalo".to_string(),
            name: "Zalo".to_string(),
            capabilities: vec![
                ChannelCap::Inbound,
                ChannelCap::Outbound,
                ChannelCap::Health,
            ],
            message_limit: Some(2000),
            supports_streaming: false,
            supports_threads: false,
            supports_reactions: false,
        }
    }

    async fn start(&self, _ctx: ChannelContext) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_CONNECTED, Ordering::SeqCst);
        tracing::info!(channel = "zalo", "ZaloAdapter started");
        Ok(())
    }

    async fn stop(&self) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_DISCONNECTED, Ordering::SeqCst);
        tracing::info!(channel = "zalo", "ZaloAdapter stopped");
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
impl Outbound for ZaloAdapter {
    async fn send(
        &self,
        envelope: &CoreMessageEnvelope,
    ) -> Result<(), synaptic::core::SynapticError> {
        // Derive user_id from thread_id ("user:<id>") or channel_id.
        let raw = envelope
            .thread_id
            .as_deref()
            .unwrap_or(envelope.channel_id.as_str());
        let user_id = raw.strip_prefix("user:").unwrap_or(raw);

        self.client
            .post("https://openapi.zalo.me/v3.0/oa/message/cs")
            .header("access_token", &self.access_token)
            .json(&serde_json::json!({
                "recipient": { "user_id": user_id },
                "message": { "text": envelope.content }
            }))
            .send()
            .await
            .map_err(|e| synaptic::core::SynapticError::Tool(e.to_string()))?;
        Ok(())
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelHealth for ZaloAdapter {
    async fn health_check(&self) -> HealthStatus {
        // Zalo OA does not expose a simple ping endpoint; report last known state.
        match self.status.load(Ordering::SeqCst) {
            STATUS_CONNECTED => HealthStatus::Healthy,
            STATUS_ERROR => HealthStatus::Unhealthy("adapter in error state".to_string()),
            _ => HealthStatus::Unhealthy("adapter not started".to_string()),
        }
    }
}
