use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;
use sha2::Digest;

use crate::gateway::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/dashboard/nodes", get(get_nodes))
        .route("/dashboard/nodes/approve", post(approve_node))
        .route("/dashboard/nodes/reject", post(reject_node))
        .route("/dashboard/nodes/remove", post(remove_node))
        .route("/dashboard/nodes/rename", post(rename_node))
        .route("/dashboard/nodes/rotate", post(rotate_node_token))
        .route("/dashboard/nodes/revoke", post(revoke_node_token))
        .route("/dashboard/nodes/qr", post(generate_qr))
        .route("/dashboard/exec-approvals", get(get_exec_approvals))
}

// ---------------------------------------------------------------------------
// GET /api/dashboard/nodes
// ---------------------------------------------------------------------------

#[derive(serde::Serialize)]
struct NodesResponse {
    nodes: Vec<serde_json::Value>,
    pending: Vec<serde_json::Value>,
}

async fn get_nodes(State(state): State<AppState>) -> Json<NodesResponse> {
    let paired = {
        let pairing = state.network.pairing_store.read().await;
        pairing.list_paired()
    };

    let registry = state.network.node_registry.read().await;
    let nodes: Vec<serde_json::Value> = paired
        .iter()
        .map(|n| {
            let session = registry.get(&n.node_id);
            let online = session.is_some();
            let token_status = match &n.token_hash {
                Some(h) if h.is_empty() => "revoked",
                Some(_) => "active",
                None => "none",
            };
            serde_json::json!({
                "id": n.node_id,
                "name": n.name,
                "platform": n.platform,
                "status": if online { "online" } else { "offline" },
                "paired_at": n.paired_at.to_string(),
                "device_id": n.device_id,
                "token_status": token_status,
                "connected_at": session.map(|s| s.connected_at),
                "capabilities": session.map(|s| &s.capabilities),
            })
        })
        .collect();
    drop(registry);

    let mut pairing_w = state.network.pairing_store.write().await;
    let pending_list = pairing_w.list_pending();
    let pending: Vec<serde_json::Value> = pending_list
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.request_id,
                "node_name": r.node_name,
                "platform": r.platform,
                "ip": r.ip,
                "requested_at": r.created_at.to_string(),
            })
        })
        .collect();

    Json(NodesResponse { nodes, pending })
}

// ---------------------------------------------------------------------------
// Node action types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct NodeActionRequest {
    request_id: Option<String>,
    node_id: Option<String>,
}

// ---------------------------------------------------------------------------
// POST /api/dashboard/nodes/approve
// ---------------------------------------------------------------------------

async fn approve_node(
    State(state): State<AppState>,
    Json(body): Json<NodeActionRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let request_id = body
        .request_id
        .as_deref()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing request_id".to_string()))?;
    let paired = state
        .network
        .pairing_store
        .write()
        .await
        .approve(request_id)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                "pending request not found".to_string(),
            )
        })?;
    Ok(Json(serde_json::to_value(&paired).unwrap_or_default()))
}

// ---------------------------------------------------------------------------
// POST /api/dashboard/nodes/reject
// ---------------------------------------------------------------------------

async fn reject_node(
    State(state): State<AppState>,
    Json(body): Json<NodeActionRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let request_id = body
        .request_id
        .as_deref()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing request_id".to_string()))?;
    let removed = state.network.pairing_store.write().await.reject(request_id);
    if removed {
        Ok(Json(serde_json::json!({"ok": true})))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            "pending request not found".to_string(),
        ))
    }
}

// ---------------------------------------------------------------------------
// POST /api/dashboard/nodes/remove
// ---------------------------------------------------------------------------

async fn remove_node(
    State(state): State<AppState>,
    Json(body): Json<NodeActionRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let node_id = body
        .node_id
        .as_deref()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing node_id".to_string()))?;
    let removed = state
        .network
        .pairing_store
        .write()
        .await
        .remove_paired(node_id);
    if removed {
        state
            .network
            .node_registry
            .write()
            .await
            .unregister(node_id);
        Ok(Json(serde_json::json!({"ok": true})))
    } else {
        Err((StatusCode::NOT_FOUND, "paired device not found".to_string()))
    }
}

// ---------------------------------------------------------------------------
// POST /api/dashboard/nodes/rename
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct RenameRequest {
    node_id: String,
    name: String,
}

async fn rename_node(
    State(state): State<AppState>,
    Json(body): Json<RenameRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let renamed = state
        .network
        .pairing_store
        .write()
        .await
        .rename(&body.node_id, &body.name);
    if renamed {
        state
            .network
            .node_registry
            .write()
            .await
            .rename(&body.node_id, &body.name);
        Ok(Json(serde_json::json!({"ok": true})))
    } else {
        Err((StatusCode::NOT_FOUND, "device not found".to_string()))
    }
}

// ---------------------------------------------------------------------------
// POST /api/dashboard/nodes/rotate
// ---------------------------------------------------------------------------

async fn rotate_node_token(
    State(state): State<AppState>,
    Json(body): Json<NodeActionRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    use crate::gateway::nodes::bootstrap;

    let node_id = body
        .node_id
        .as_deref()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing node_id".to_string()))?;

    let new_token = bootstrap::generate_pairing_token();
    let token_hash = format!("{:x}", sha2::Sha256::digest(new_token.as_bytes()));

    let updated = state
        .network
        .pairing_store
        .write()
        .await
        .update_token_hash(node_id, &token_hash);
    if updated {
        Ok(Json(serde_json::json!({
            "ok": true,
            "token": new_token,
        })))
    } else {
        Err((StatusCode::NOT_FOUND, "device not found".to_string()))
    }
}

// ---------------------------------------------------------------------------
// POST /api/dashboard/nodes/revoke
// ---------------------------------------------------------------------------

async fn revoke_node_token(
    State(state): State<AppState>,
    Json(body): Json<NodeActionRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let node_id = body
        .node_id
        .as_deref()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing node_id".to_string()))?;

    let updated = state
        .network
        .pairing_store
        .write()
        .await
        .update_token_hash(node_id, "");
    if updated {
        Ok(Json(serde_json::json!({"ok": true})))
    } else {
        Err((StatusCode::NOT_FOUND, "device not found".to_string()))
    }
}

// ---------------------------------------------------------------------------
// POST /api/dashboard/nodes/qr
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct QrRequest {
    url: Option<String>,
}

async fn generate_qr(
    State(state): State<AppState>,
    Json(body): Json<QrRequest>,
) -> Json<serde_json::Value> {
    use crate::gateway::nodes::bootstrap;

    let token = state.network.bootstrap_store.write().await.issue();

    let gateway_url = body.url.unwrap_or_else(|| {
        let port = state
            .core
            .config
            .serve
            .as_ref()
            .and_then(|s| s.port)
            .unwrap_or(3000);
        format!("ws://localhost:{}", port)
    });

    let setup_code = bootstrap::encode_setup_code(&gateway_url, &token);
    let qr_svg = bootstrap::generate_qr_svg(&setup_code).unwrap_or_default();

    Json(serde_json::json!({
        "ok": true,
        "setup_code": setup_code,
        "qr_svg": qr_svg,
        "gateway_url": gateway_url,
        "bootstrap_token": token,
        "ttl_ms": 10 * 60 * 1000,
    }))
}

// ---------------------------------------------------------------------------
// GET /api/dashboard/exec-approvals
// ---------------------------------------------------------------------------

async fn get_exec_approvals(State(state): State<AppState>) -> Json<serde_json::Value> {
    let config = state.channel.exec_approvals_config.read().await;
    Json(serde_json::json!({
        "security_mode": config.mode,
        "ask_policy": config.ask,
        "allowlist": config.allowlist,
    }))
}
