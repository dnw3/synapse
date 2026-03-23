//! RPC handlers for debug operations.

use std::sync::Arc;

use serde_json::{json, Value};

use super::router::RpcContext;
use super::types::RpcError;

// ---------------------------------------------------------------------------
// debug.invoke
// ---------------------------------------------------------------------------

pub async fn handle_invoke(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let method = params
        .get("method")
        .or_else(|| params.get("prompt"))
        .and_then(|v| v.as_str())
        .unwrap_or("health");

    match method {
        "health" => {
            let uptime = ctx.state.core.started_at.elapsed().as_secs();
            let active = ctx.state.session.cancel_tokens.read().await.len();
            Ok(json!({
                "ok": true,
                "result": {
                    "status": "ok",
                    "uptime_secs": uptime,
                    "active_connections": active,
                },
            }))
        }
        "cost_snapshot" => {
            let snapshot = ctx.state.agent.cost_tracker.snapshot().await;
            Ok(json!({
                "ok": true,
                "result": {
                    "total_input_tokens": snapshot.total_input_tokens,
                    "total_output_tokens": snapshot.total_output_tokens,
                    "total_cost_usd": snapshot.estimated_cost_usd,
                    "total_requests": snapshot.total_requests,
                },
            }))
        }
        "stats" => {
            let snapshot = ctx.state.agent.cost_tracker.snapshot().await;
            let sessions = ctx
                .state
                .session.sessions
                .list_sessions()
                .await
                .map(|s| s.len())
                .unwrap_or(0);
            let active = ctx.state.session.cancel_tokens.read().await.len();
            Ok(json!({
                "ok": true,
                "result": {
                    "session_count": sessions,
                    "total_input_tokens": snapshot.total_input_tokens,
                    "total_output_tokens": snapshot.total_output_tokens,
                    "total_cost_usd": snapshot.estimated_cost_usd,
                    "total_requests": snapshot.total_requests,
                    "active_ws_sessions": active,
                    "uptime_secs": ctx.state.core.started_at.elapsed().as_secs(),
                },
            }))
        }
        "version" => Ok(json!({
            "ok": true,
            "result": {
                "version": env!("CARGO_PKG_VERSION"),
                "name": env!("CARGO_PKG_NAME"),
            },
        })),
        _ => Ok(json!({
            "ok": false,
            "error": format!("unknown method: {}. Available: health, cost_snapshot, stats, version", method),
        })),
    }
}

// ---------------------------------------------------------------------------
// debug.health
// ---------------------------------------------------------------------------

pub async fn handle_health(ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    let active = ctx.state.session.cancel_tokens.read().await.len();
    let sessions = ctx
        .state
        .session.sessions
        .list_sessions()
        .await
        .map(|s| s.len())
        .unwrap_or(0);

    let memory_rss_mb: Option<f64> = {
        #[cfg(unix)]
        {
            std::fs::read_to_string("/proc/self/statm")
                .ok()
                .and_then(|s| s.split_whitespace().nth(1)?.parse::<u64>().ok())
                .map(|pages| (pages * 4096) as f64 / (1024.0 * 1024.0))
        }
        #[cfg(not(unix))]
        {
            None
        }
    };

    Ok(json!({
        "status": "ok",
        "uptime_secs": ctx.state.core.started_at.elapsed().as_secs(),
        "memory_rss_mb": memory_rss_mb,
        "active_connections": active,
        "active_sessions": sessions,
    }))
}
