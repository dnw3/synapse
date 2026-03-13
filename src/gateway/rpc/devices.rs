//! RPC handlers for device pairing and token management.

use std::sync::Arc;

use serde_json::{json, Value};

use super::router::RpcContext;
use super::types::RpcError;

pub async fn handle_pair_approve(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let request_id = params
        .get("request_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing request_id"))?;
    let paired = ctx
        .state
        .pairing_store
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
    let removed = ctx.state.pairing_store.write().await.reject(request_id);
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
    let removed = ctx.state.pairing_store.write().await.remove_paired(node_id);
    if removed {
        // Also unregister from live registry
        ctx.state.node_registry.write().await.unregister(node_id);
        Ok(json!({"ok": true}))
    } else {
        Err(RpcError::not_found("paired device not found"))
    }
}

pub async fn handle_pair_list(ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    let paired = ctx.state.pairing_store.read().await.list_paired();
    Ok(serde_json::to_value(&paired).unwrap_or_default())
}

pub async fn handle_token_rotate(_ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    // Stub: token rotation not yet implemented
    let _node_id = params
        .get("node_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing node_id"))?;
    Ok(json!({"ok": true, "message": "token rotation not yet implemented"}))
}

pub async fn handle_token_revoke(_ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    // Stub: token revocation not yet implemented
    let _node_id = params
        .get("node_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing node_id"))?;
    Ok(json!({"ok": true, "message": "token revocation not yet implemented"}))
}
