use std::sync::atomic::{AtomicU8, Ordering};
use std::sync::Arc;

use async_trait::async_trait;
use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use serde::Deserialize;

use crate::agent;
use crate::channels::handler::AgentSession;
use crate::config::{SynapseConfig, SynologyBotConfig};
use crate::gateway::messages::{ChannelInfo, ChatInfo, InboundMessage, SenderInfo};
use synaptic::core::{
    ChannelAdapter, ChannelCap, ChannelContext, ChannelHealth, ChannelManifest, ChannelStatus,
    HealthStatus, MessageEnvelope as CoreMessageEnvelope, Outbound, RunContext,
};

#[derive(Clone)]
struct AppState {
    agent_session: Arc<AgentSession>,
    allowlist: crate::config::BotAllowlist,
    outgoing_url: Option<String>,
}

#[derive(Deserialize)]
struct SynologyWebhook {
    #[serde(default)]
    user_id: Option<String>,
    #[serde(default)]
    channel_id: Option<String>,
    #[serde(default)]
    text: Option<String>,
    #[serde(default)]
    token: Option<String>,
}

/// Run the Synology Chat bot adapter (incoming webhook mode).
pub async fn run(
    config: &SynapseConfig,
    model_override: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let syn_configs: Vec<crate::config::SynologyBotConfig> = config.channel_configs("synology");
    let syn_config = syn_configs
        .first()
        .ok_or("missing [[channels.synology]] section in config")?;

    let model = agent::build_model(config, model_override)?;
    let config_arc = Arc::new(config.clone());
    let agent_session = Arc::new(AgentSession::new(model, config_arc, true));

    let port = syn_config.port.unwrap_or(8091);

    let state = AppState {
        agent_session,
        allowlist: syn_config.allowlist.clone(),
        outgoing_url: syn_config.outgoing_webhook_url.clone(),
    };

    let app = Router::new()
        .route("/webhook", post(handle_webhook))
        .with_state(state);

    let addr = format!("0.0.0.0:{}", port);
    tracing::info!(channel = "synology", addr = %addr, "adapter started");

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn handle_webhook(
    State(state): State<AppState>,
    Json(payload): Json<SynologyWebhook>,
) -> impl IntoResponse {
    let text = payload.text.unwrap_or_default();
    if text.is_empty() {
        return (StatusCode::OK, Json(serde_json::json!({"text": ""})));
    }

    if !state
        .allowlist
        .is_allowed(payload.user_id.as_deref(), payload.channel_id.as_deref())
    {
        return (
            StatusCode::FORBIDDEN,
            Json(serde_json::json!({"text": "not allowed"})),
        );
    }

    let syn_user_id = payload.user_id.clone();
    let syn_channel_id = payload.channel_id.clone();
    let session_key = syn_channel_id
        .clone()
        .unwrap_or_else(|| syn_user_id.clone().unwrap_or_else(|| "default".to_string()));

    let channel_info = ChannelInfo {
        platform: "synology".into(),
        native_channel_id: syn_channel_id.clone(),
        ..Default::default()
    };
    let sender_info = SenderInfo {
        id: syn_user_id.clone(),
        ..Default::default()
    };
    let chat_info = ChatInfo {
        chat_type: if syn_channel_id.is_some() {
            "group"
        } else {
            "direct"
        }
        .to_string(),
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
    match state
        .agent_session
        .handle_message(msg, RunContext::default())
        .await
    {
        Ok(reply) => {
            // If outgoing webhook URL is configured, send there
            if let Some(ref url) = state.outgoing_url {
                let client = reqwest::Client::new();
                let _ = client
                    .post(url)
                    .json(&serde_json::json!({"text": reply.content}))
                    .send()
                    .await;
            }
            (
                StatusCode::OK,
                Json(serde_json::json!({"text": reply.content})),
            )
        }
        Err(e) => {
            tracing::error!(channel = "synology", error = %e, "agent error");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"text": format!("Error: {}", e)})),
            )
        }
    }
}

// ---------------------------------------------------------------------------
// ChannelAdapter / Outbound / ChannelHealth trait implementations
// ---------------------------------------------------------------------------

/// Status constants used by [`SynologyAdapter`].
const STATUS_DISCONNECTED: u8 = 0;
const STATUS_CONNECTED: u8 = 1;
const STATUS_ERROR: u8 = 2;

/// Channel adapter facade for Synology Chat (incoming webhook mode).
#[allow(dead_code)]
pub struct SynologyAdapter {
    /// Optional URL to post outgoing messages back to Synology Chat.
    outgoing_url: Option<String>,
    client: reqwest::Client,
    /// Atomic status: 0 = Disconnected, 1 = Connected, 2 = Error.
    status: AtomicU8,
}

#[allow(dead_code)]
impl SynologyAdapter {
    pub fn new(outgoing_url: Option<String>) -> Self {
        Self {
            outgoing_url,
            client: reqwest::Client::new(),
            status: AtomicU8::new(STATUS_DISCONNECTED),
        }
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelAdapter for SynologyAdapter {
    fn manifest(&self) -> ChannelManifest {
        ChannelManifest {
            id: "synology".to_string(),
            name: "Synology Chat".to_string(),
            capabilities: vec![
                ChannelCap::Inbound,
                ChannelCap::Outbound,
                ChannelCap::Health,
            ],
            message_limit: Some(10000),
            supports_streaming: false,
            supports_threads: false,
            supports_reactions: false,
        }
    }

    async fn start(&self, _ctx: ChannelContext) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_CONNECTED, Ordering::SeqCst);
        tracing::info!(channel = "synology", "SynologyAdapter started");
        Ok(())
    }

    async fn stop(&self) -> Result<(), synaptic::core::SynapticError> {
        self.status.store(STATUS_DISCONNECTED, Ordering::SeqCst);
        tracing::info!(channel = "synology", "SynologyAdapter stopped");
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
impl Outbound for SynologyAdapter {
    async fn send(
        &self,
        envelope: &CoreMessageEnvelope,
    ) -> Result<(), synaptic::core::SynapticError> {
        if let Some(ref url) = self.outgoing_url {
            self.client
                .post(url)
                .json(&serde_json::json!({ "text": envelope.content }))
                .send()
                .await
                .map_err(|e| synaptic::core::SynapticError::Tool(e.to_string()))?;
        }
        // No outgoing_url configured — message is returned inline via HTTP response.
        Ok(())
    }
}

#[allow(dead_code)]
#[async_trait]
impl ChannelHealth for SynologyAdapter {
    async fn health_check(&self) -> HealthStatus {
        // Synology Chat does not expose a dedicated status endpoint; report
        // the last known connection state.
        match self.status.load(Ordering::SeqCst) {
            STATUS_CONNECTED => HealthStatus::Healthy,
            STATUS_ERROR => HealthStatus::Unhealthy("adapter in error state".to_string()),
            _ => HealthStatus::Unhealthy("adapter not started".to_string()),
        }
    }
}
