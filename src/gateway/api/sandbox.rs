use axum::{
    extract::{Path, Query, State},
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};

use crate::gateway::state::AppState;

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct SandboxListParams {
    pub provider: Option<String>,
}

#[derive(Deserialize)]
pub struct SandboxExplainParams {
    pub session: Option<String>,
    pub agent: Option<String>,
}

#[derive(Deserialize)]
#[allow(dead_code)]
pub struct SandboxRecreateRequest {
    pub all: Option<bool>,
    pub session: Option<String>,
    pub agent: Option<String>,
}

#[derive(Serialize)]
pub struct SandboxRecreateResponse {
    pub count: u32,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/sandbox", get(list_sandboxes))
        .route("/sandbox/explain", get(explain_sandbox))
        .route("/sandbox/recreate", post(recreate_sandbox))
        .route("/sandbox/{runtime_id}", delete(destroy_sandbox))
        .route("/sandbox/providers", get(list_providers))
}

async fn list_sandboxes(
    State(_state): State<AppState>,
    Query(_params): Query<SandboxListParams>,
) -> Json<Vec<serde_json::Value>> {
    // TODO: wire to orchestrator when available in AppState
    Json(vec![])
}

async fn explain_sandbox(
    State(_state): State<AppState>,
    Query(params): Query<SandboxExplainParams>,
) -> Json<serde_json::Value> {
    let session = params.session.as_deref().unwrap_or("main");
    let agent = params.agent.as_deref().unwrap_or("main");
    // TODO: wire to orchestrator
    Json(serde_json::json!({
        "session_key": session,
        "agent_id": agent,
        "is_sandboxed": false,
        "mode": "off"
    }))
}

async fn recreate_sandbox(
    State(_state): State<AppState>,
    Json(_body): Json<SandboxRecreateRequest>,
) -> Json<SandboxRecreateResponse> {
    // TODO: wire to orchestrator
    Json(SandboxRecreateResponse { count: 0 })
}

async fn destroy_sandbox(
    State(_state): State<AppState>,
    Path(_runtime_id): Path<String>,
) -> Json<serde_json::Value> {
    // TODO: wire to orchestrator
    Json(serde_json::json!({"ok": true}))
}

async fn list_providers(State(_state): State<AppState>) -> Json<Vec<String>> {
    // TODO: wire to registry
    Json(vec![])
}
