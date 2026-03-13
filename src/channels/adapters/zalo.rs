use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use serde::Deserialize;

use synaptic::DeliveryContext;

use crate::agent;
use crate::channels::handler::AgentSession;
use crate::config::bot::resolve_secret;
use crate::config::{SynapseConfig, ZaloBotConfig};
use crate::gateway::messages::MessageEnvelope;

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
        .as_ref()
        .ok_or("missing [zalo] section in config")?;

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
        let envelope = MessageEnvelope::channel(
            sender_id.clone(),
            text.clone(),
            DeliveryContext {
                channel: "zalo".into(),
                to: Some(format!("user:{}", sender_id)),
                account_id: None,
                thread_id: None,
                meta: None,
            },
        );
        match session.handle_message(envelope).await {
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
