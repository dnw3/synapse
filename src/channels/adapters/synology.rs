use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use serde::Deserialize;

use synaptic::DeliveryContext;

use crate::agent;
use crate::channels::handler::AgentSession;
use crate::config::{SynapseConfig, SynologyBotConfig};
use crate::gateway::messages::MessageEnvelope;

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
    let syn_config = config
        .synology
        .first()
        .ok_or("missing [[synology]] section in config")?;

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

    let mut envelope = MessageEnvelope::channel(
        session_key.clone(),
        text.clone(),
        DeliveryContext {
            channel: "synology".into(),
            to: Some(format!("chat:{}", session_key)),
            account_id: None,
            thread_id: None,
            meta: None,
        },
    );
    if let Some(ref uid) = syn_user_id {
        envelope.sender_id = Some(uid.clone());
    }
    envelope.routing.peer_kind = Some(if syn_channel_id.is_some() {
        crate::config::PeerKind::Group
    } else {
        crate::config::PeerKind::Direct
    });
    envelope.routing.peer_id = Some(session_key.clone());
    match state.agent_session.handle_message(envelope).await {
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
