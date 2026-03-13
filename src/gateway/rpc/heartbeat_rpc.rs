//! Heartbeat RPC stubs.

use std::sync::Arc;

use serde_json::{json, Value};

use super::router::RpcContext;
use super::types::RpcError;
use crate::gateway::presence::now_ms;

pub async fn handle_last_heartbeat(
    _ctx: Arc<RpcContext>,
    _params: Value,
) -> Result<Value, RpcError> {
    Ok(json!({
        "ts": now_ms(),
        "status": "ok",
    }))
}

pub async fn handle_set_heartbeats(
    _ctx: Arc<RpcContext>,
    _params: Value,
) -> Result<Value, RpcError> {
    // Stub: heartbeat interval configuration
    Ok(json!({"ok": true}))
}
