//! Health and status RPC method handlers.

use std::sync::Arc;
use std::time::Instant;

use serde_json::{json, Value};

use super::router::RpcContext;
use super::types::RpcError;

/// Handle the `health` RPC method.
///
/// Returns a simple health check with uptime and response duration.
pub async fn handle_health(ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    let start = Instant::now();
    let uptime_secs = ctx.state.core.started_at.elapsed().as_secs();
    let duration = start.elapsed().as_micros() as f64 / 1000.0; // ms

    Ok(json!({
        "ok": true,
        "uptime_secs": uptime_secs,
        "duration_ms": duration,
    }))
}

/// Handle the `status` RPC method.
///
/// Returns server status including auth state and connection count.
pub async fn handle_status(ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    let uptime_secs = ctx.state.core.started_at.elapsed().as_secs();
    let auth_enabled = ctx
        .state
        .core
        .auth
        .as_ref()
        .map(|a| a.config.enabled)
        .unwrap_or(false);
    let connections = ctx.broadcaster.connection_count().await;

    Ok(json!({
        "status": "ok",
        "uptime_secs": uptime_secs,
        "auth_enabled": auth_enabled,
        "connections": connections,
    }))
}
