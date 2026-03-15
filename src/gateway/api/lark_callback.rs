use axum::{extract::State, http::StatusCode, routing::post, Json, Router};
use serde_json::{json, Value};

use crate::gateway::state::AppState;

async fn handle_card_callback(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> Result<Json<Value>, (StatusCode, String)> {
    // Handle URL verification challenge (Lark sends this on callback registration)
    if let Some(challenge) = body.get("challenge").and_then(|v| v.as_str()) {
        return Ok(Json(json!({ "challenge": challenge })));
    }

    // Parse action value from the card callback
    let action_value = body
        .pointer("/action/value")
        .and_then(|v| v.as_str())
        .unwrap_or("");

    // Parse action: "pair_approve:{request_id}" or "pair_reject:{request_id}"
    let (operation, request_id) = action_value
        .split_once(':')
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "invalid action format".to_string()))?;

    match operation {
        "pair_approve" => {
            let result = state.pairing_store.write().await.approve(request_id);
            if result.is_some() {
                Ok(Json(json!({
                    "toast": { "type": "success", "content": "Device approved" }
                })))
            } else {
                Ok(Json(json!({
                    "toast": { "type": "info", "content": "Request not found or already resolved" }
                })))
            }
        }
        "pair_reject" => {
            let rejected = state.pairing_store.write().await.reject(request_id);
            if rejected {
                Ok(Json(json!({
                    "toast": { "type": "info", "content": "Device rejected" }
                })))
            } else {
                Ok(Json(json!({
                    "toast": { "type": "info", "content": "Request not found or already resolved" }
                })))
            }
        }
        _ => Err((
            StatusCode::BAD_REQUEST,
            format!("unknown operation: {operation}"),
        )),
    }
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/lark/card_callback", post(handle_card_callback))
}
