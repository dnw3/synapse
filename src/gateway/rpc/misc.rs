//! Miscellaneous RPC stubs.

use std::sync::Arc;

use serde_json::{json, Value};

use super::router::RpcContext;
use super::types::RpcError;

pub async fn handle_send(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    // Forward a message to connected clients
    ctx.broadcaster
        .broadcast(
            "message",
            json!({
                "from": ctx.conn_id,
                "data": params.get("data").cloned().unwrap_or(Value::Null),
            }),
        )
        .await;
    Ok(json!({"ok": true}))
}

pub async fn handle_wake(ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    // Broadcast a wake event to all connected clients
    ctx.broadcaster
        .broadcast("wake", json!({"ts": crate::gateway::presence::now_ms()}))
        .await;
    Ok(json!({"ok": true}))
}

pub async fn handle_updates_run(_ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    // Stub: trigger update check
    Ok(json!({
        "ok": true,
        "current_version": env!("CARGO_PKG_VERSION"),
        "latest_version": null,
        "update_available": false,
    }))
}

pub async fn handle_doctor_memory_status(
    _ctx: Arc<RpcContext>,
    _params: Value,
) -> Result<Value, RpcError> {
    // Stub: memory diagnostics
    Ok(json!({
        "status": "ok",
        "heap_mb": null,
        "rss_mb": null,
    }))
}
