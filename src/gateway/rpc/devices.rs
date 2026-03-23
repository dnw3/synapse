//! RPC handlers for device pairing, token management, and QR code generation.

use std::sync::Arc;

use serde_json::{json, Value};

use super::router::RpcContext;
use super::types::RpcError;
use sha2::Digest;

use crate::gateway::nodes::bootstrap;
use crate::gateway::presence::now_ms;

// ---------------------------------------------------------------------------
// Pairing
// ---------------------------------------------------------------------------

pub async fn handle_pair_approve(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let request_id = params
        .get("request_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing request_id"))?;
    let paired = ctx
        .state
        .network.pairing_store
        .write()
        .await
        .approve(request_id)
        .ok_or_else(|| RpcError::not_found("pending request not found"))?;
    Ok(serde_json::to_value(&paired).unwrap_or_default())
}

pub async fn handle_pair_reject(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let request_id = params
        .get("request_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing request_id"))?;
    let removed = ctx.state.network.pairing_store.write().await.reject(request_id);
    if removed {
        Ok(json!({"ok": true}))
    } else {
        Err(RpcError::not_found("pending request not found"))
    }
}

pub async fn handle_pair_remove(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let node_id = params
        .get("node_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing node_id"))?;
    let removed = ctx.state.network.pairing_store.write().await.remove_paired(node_id);
    if removed {
        // Also unregister from live registry
        ctx.state.network.node_registry.write().await.unregister(node_id);
        Ok(json!({"ok": true}))
    } else {
        Err(RpcError::not_found("paired device not found"))
    }
}

pub async fn handle_pair_list(ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    let paired = ctx.state.network.pairing_store.read().await.list_paired();
    Ok(serde_json::to_value(&paired).unwrap_or_default())
}

// ---------------------------------------------------------------------------
// Token Management
// ---------------------------------------------------------------------------

pub async fn handle_token_rotate(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let node_id = params
        .get("node_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing node_id"))?;

    let mut store = ctx.state.network.pairing_store.write().await;
    let paired = store
        .list_paired()
        .into_iter()
        .find(|n| n.node_id == node_id);

    let Some(_node) = paired else {
        return Err(RpcError::not_found("paired node not found"));
    };

    // Generate new token and store its hash
    let new_token = bootstrap::generate_pairing_token();
    let hash = sha2::Sha256::digest(new_token.as_bytes());
    let token_hash = format!("{:x}", hash);

    // Update the pairing store with new token hash
    // We need to find and update the node's token_hash
    let updated = store.update_token_hash(node_id, &token_hash);
    if !updated {
        return Err(RpcError::internal("failed to update token"));
    }

    Ok(json!({
        "ok": true,
        "node_id": node_id,
        "token": new_token,
        "rotated_at_ms": now_ms(),
    }))
}

pub async fn handle_token_revoke(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let node_id = params
        .get("node_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing node_id"))?;

    let mut store = ctx.state.network.pairing_store.write().await;

    // Clear the token hash (revoke access)
    let updated = store.update_token_hash(node_id, "");
    if !updated {
        return Err(RpcError::not_found("paired node not found"));
    }

    Ok(json!({
        "ok": true,
        "node_id": node_id,
        "revoked_at_ms": now_ms(),
    }))
}

// ---------------------------------------------------------------------------
// Bootstrap Token & QR Code
// ---------------------------------------------------------------------------

/// Issue a new bootstrap token for device pairing.
pub async fn handle_bootstrap_issue(
    ctx: Arc<RpcContext>,
    _params: Value,
) -> Result<Value, RpcError> {
    let token = ctx.state.network.bootstrap_store.write().await.issue();
    Ok(json!({
        "ok": true,
        "token": token,
        "ttl_ms": 10 * 60 * 1000,
    }))
}

/// Verify a bootstrap token (called by the connecting device).
pub async fn handle_bootstrap_verify(
    ctx: Arc<RpcContext>,
    params: Value,
) -> Result<Value, RpcError> {
    let token = params
        .get("token")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing token"))?;
    let device_id = params
        .get("device_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing device_id"))?;
    let public_key = params
        .get("public_key")
        .and_then(|v| v.as_str())
        .unwrap_or("");
    let role = params
        .get("role")
        .and_then(|v| v.as_str())
        .unwrap_or("client");
    let scopes: Vec<String> = params
        .get("scopes")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|s| s.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();

    let valid = ctx
        .state
        .network.bootstrap_store
        .write()
        .await
        .verify(token, device_id, public_key, role, &scopes);

    if valid {
        Ok(json!({"ok": true, "verified": true}))
    } else {
        Err(RpcError::invalid_request(
            "invalid or expired bootstrap token",
        ))
    }
}

/// List active bootstrap tokens.
pub async fn handle_bootstrap_list(
    ctx: Arc<RpcContext>,
    _params: Value,
) -> Result<Value, RpcError> {
    let tokens = ctx.state.network.bootstrap_store.write().await.list();
    Ok(serde_json::to_value(&tokens).unwrap_or_default())
}

/// Generate a QR code + setup code for device pairing.
/// Returns: `{ setup_code, qr_svg, gateway_url }`
pub async fn handle_qr_generate(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    // Issue a bootstrap token
    let token = ctx.state.network.bootstrap_store.write().await.issue();

    // Resolve gateway URL
    let gateway_url = params
        .get("url")
        .and_then(|v| v.as_str())
        .map(String::from)
        .unwrap_or_else(|| {
            // Try to construct from config
            let port = ctx
                .state
                .core.config
                .serve
                .as_ref()
                .and_then(|s| s.port)
                .unwrap_or(3000);
            format!("ws://localhost:{}", port)
        });

    // Encode setup code
    let setup_code = bootstrap::encode_setup_code(&gateway_url, &token);

    // Generate QR SVG
    let qr_svg = bootstrap::generate_qr_svg(&setup_code).unwrap_or_default();

    Ok(json!({
        "ok": true,
        "setup_code": setup_code,
        "qr_svg": qr_svg,
        "gateway_url": gateway_url,
        "bootstrap_token": token,
        "ttl_ms": 10 * 60 * 1000,
    }))
}

/// Decode a setup code (utility for clients).
pub async fn handle_setup_code_decode(
    _ctx: Arc<RpcContext>,
    params: Value,
) -> Result<Value, RpcError> {
    let code = params
        .get("code")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing code"))?;
    let (url, token) = bootstrap::decode_setup_code(code)
        .ok_or_else(|| RpcError::invalid_request("invalid setup code"))?;
    Ok(json!({
        "url": url,
        "bootstrapToken": token,
    }))
}
