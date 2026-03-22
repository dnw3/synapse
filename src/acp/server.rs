//! HTTP/WebSocket transport for ACP — mounts on the gateway server.

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::post;
use axum::Router;
use synaptic::core::MemoryStore;
use synaptic::deep::acp::handler::AcpHandler;
use synaptic::deep::acp::types::*;

use crate::gateway::state::AppState;

/// Create ACP HTTP routes to be merged into the gateway.
pub fn routes() -> Router<AppState> {
    Router::new().route("/acp", post(handle_acp))
}

async fn handle_acp(
    State(state): State<AppState>,
    Json(req): Json<JsonRpcRequest>,
) -> Result<Json<JsonRpcResponse>, (StatusCode, String)> {
    let handler = AcpHandler::new("synapse", env!("CARGO_PKG_VERSION"));

    // Try framework-level routing first
    if let Some(resp) = handler.route(&req) {
        return Ok(Json(resp));
    }

    // Handle agent methods
    match req.method.as_str() {
        "agent/run" => {
            let run_params: AgentRunParams = req
                .params
                .and_then(|p| serde_json::from_value(p).ok())
                .ok_or((StatusCode::BAD_REQUEST, "invalid params".to_string()))?;

            let session_id = run_params
                .session_id
                .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());

            // Simple non-streaming execution
            let memory = state.sessions.memory();
            let mut messages = memory.load(&session_id).await.unwrap_or_default();
            if messages.is_empty() {
                if let Some(ref prompt) = state.config.base.agent.system_prompt {
                    messages.push(synaptic::core::Message::system(prompt));
                }
            }
            messages.push(synaptic::core::Message::human(&run_params.task));

            let cwd = std::env::current_dir().unwrap_or_else(|_| ".".into());
            let checkpointer = std::sync::Arc::new(state.sessions.checkpointer());
            let agent = crate::agent::build_deep_agent(
                state.model.clone(),
                &state.config,
                &cwd,
                checkpointer,
                vec![],
                None,
                crate::agent::SessionKind::Full,
            )
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

            let initial = synaptic::graph::MessageState::with_messages(messages);
            let result = agent
                .invoke(initial)
                .await
                .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

            let final_state = result.into_state();
            let content = final_state
                .messages
                .iter()
                .rev()
                .find(|m| m.is_ai() && !m.content().is_empty())
                .map(|m| m.content().to_string())
                .unwrap_or_default();

            Ok(Json(JsonRpcResponse::success(
                req.id,
                serde_json::json!({
                    "session_id": session_id,
                    "content": content,
                }),
            )))
        }
        "agent/status" => Ok(Json(JsonRpcResponse::success(
            req.id,
            serde_json::json!({"status": "idle"}),
        ))),
        "agent/cancel" => Ok(Json(JsonRpcResponse::success(
            req.id,
            serde_json::json!({"cancelled": true}),
        ))),
        _ => Ok(Json(JsonRpcResponse::error(
            req.id,
            METHOD_NOT_FOUND,
            "method not found",
        ))),
    }
}
