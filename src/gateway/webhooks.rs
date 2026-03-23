//! Webhook endpoints — trigger agent runs via HTTP POST.

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::post;
use axum::Router;
use serde::{Deserialize, Serialize};

use super::state::AppState;

#[derive(Deserialize)]
pub struct WebhookPayload {
    /// The prompt/message to send to the agent.
    pub message: String,
}

#[derive(Serialize)]
pub struct WebhookResponse {
    pub status: String,
    pub response: String,
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/api/webhooks/{name}", post(handle_webhook))
}

async fn handle_webhook(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(payload): Json<WebhookPayload>,
) -> Result<Json<WebhookResponse>, (StatusCode, String)> {
    tracing::info!(webhook = %name, "webhook triggered");

    let messages = vec![
        synaptic::core::Message::system(format!(
            "You are Synapse, handling webhook '{}'. Respond concisely.",
            name
        )),
        synaptic::core::Message::human(&payload.message),
    ];

    let request = synaptic::core::ChatRequest::new(messages);
    let response = state
        .agent.model
        .chat(request)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    Ok(Json(WebhookResponse {
        status: "ok".to_string(),
        response: response.message.content().to_string(),
    }))
}
