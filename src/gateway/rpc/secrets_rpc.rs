//! Secrets RPC stubs.

use std::sync::Arc;

use serde_json::{json, Value};

use super::router::RpcContext;
use super::types::RpcError;

pub async fn handle_reload(_ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    // Stub: reload secrets from disk/env
    Ok(json!({"ok": true, "message": "secrets reloaded"}))
}

pub async fn handle_resolve(_ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    // Stub: resolve a secret name to check availability (not value)
    let name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
    Ok(json!({
        "name": name,
        "available": false,
        "message": "secrets resolution not yet implemented",
    }))
}
