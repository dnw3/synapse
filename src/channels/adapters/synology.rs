use std::sync::Arc;

use axum::{extract::State, http::StatusCode, response::IntoResponse, routing::post, Json, Router};
use serde::Deserialize;

use crate::agent;
use crate::channels::handler::AgentSession;
use crate::config::{SynapseConfig, SynologyBotConfig};

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
        .as_ref()
        .ok_or("missing [synology] section in config")?;

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

    let session_key = payload
        .channel_id
        .unwrap_or_else(|| payload.user_id.unwrap_or_else(|| "default".to_string()));

    match state
        .agent_session
        .handle_message(&session_key, &text)
        .await
    {
        Ok(reply) => {
            // If outgoing webhook URL is configured, send there
            if let Some(ref url) = state.outgoing_url {
                let client = reqwest::Client::new();
                let _ = client
                    .post(url)
                    .json(&serde_json::json!({"text": reply}))
                    .send()
                    .await;
            }
            (StatusCode::OK, Json(serde_json::json!({"text": reply})))
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
