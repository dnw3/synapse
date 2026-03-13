//! RPC handlers for usage and cost tracking.

use std::sync::Arc;

use serde_json::{json, Value};
use synaptic::core::MemoryStore;

use super::router::RpcContext;
use super::types::RpcError;

// ---------------------------------------------------------------------------
// usage.status
// ---------------------------------------------------------------------------

pub async fn handle_status(ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    let sessions = ctx
        .state
        .sessions
        .list_sessions()
        .await
        .map_err(|e| RpcError::internal(e.to_string()))?;

    let memory = ctx.state.sessions.memory();
    let mut total_messages = 0usize;
    for s in &sessions {
        total_messages += memory.load(&s.id).await.map(|m| m.len()).unwrap_or(0);
    }

    let snapshot = ctx.state.cost_tracker.snapshot().await;
    let active_ws = ctx.state.cancel_tokens.read().await.len();

    Ok(json!({
        "session_count": sessions.len(),
        "total_messages": total_messages,
        "total_input_tokens": snapshot.total_input_tokens,
        "total_output_tokens": snapshot.total_output_tokens,
        "total_cost_usd": snapshot.estimated_cost_usd,
        "total_requests": snapshot.total_requests,
        "uptime_secs": ctx.state.started_at.elapsed().as_secs(),
        "active_ws_sessions": active_ws,
    }))
}

// ---------------------------------------------------------------------------
// usage.cost
// ---------------------------------------------------------------------------

pub async fn handle_cost(ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    let snapshot = ctx.state.cost_tracker.snapshot().await;

    let mut per_model: Vec<Value> = snapshot
        .per_model
        .into_iter()
        .map(|(model, usage)| {
            json!({
                "model": model,
                "input_tokens": usage.input_tokens,
                "output_tokens": usage.output_tokens,
                "requests": usage.requests,
                "cost_usd": usage.cost_usd,
            })
        })
        .collect();
    per_model.sort_by(|a, b| {
        a.get("model")
            .and_then(|v| v.as_str())
            .cmp(&b.get("model").and_then(|v| v.as_str()))
    });

    Ok(json!({
        "total_input_tokens": snapshot.total_input_tokens,
        "total_output_tokens": snapshot.total_output_tokens,
        "total_requests": snapshot.total_requests,
        "total_cost_usd": snapshot.estimated_cost_usd,
        "per_model": per_model,
    }))
}
