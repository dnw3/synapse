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

pub async fn handle_updates_run(ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    // Stub: trigger update check.
    // In a real implementation this would fetch the latest version from a registry.
    let current = env!("CARGO_PKG_VERSION");
    let latest_version: Option<String> = None; // placeholder until registry is integrated

    if let Some(ref latest) = latest_version {
        if latest.as_str() != current {
            ctx.broadcaster
                .broadcast(
                    "update.available",
                    json!({
                        "current": current,
                        "latest": latest,
                    }),
                )
                .await;
        }
    }

    Ok(json!({
        "ok": true,
        "current_version": current,
        "latest_version": latest_version,
        "update_available": latest_version.as_deref().map(|l| l != current).unwrap_or(false),
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
