//! RPC handlers for log queries.

use std::sync::Arc;

use serde_json::{json, Value};

use super::router::RpcContext;
use super::types::RpcError;

// ---------------------------------------------------------------------------
// logs.tail
// ---------------------------------------------------------------------------

pub async fn handle_tail(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let request_id = params.get("request_id").and_then(|v| v.as_str());
    let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(100) as usize;
    let from = params.get("from").and_then(|v| v.as_str());

    let entries = ctx
        .state
        .log_buffer
        .query(limit, None, request_id, from, None, None)
        .await;

    let lines: Vec<Value> = entries
        .into_iter()
        .map(|e| {
            json!({
                "timestamp": e.ts,
                "level": e.level,
                "target": e.target,
                "message": e.message,
                "request_id": e.request_id,
                "fields": e.fields,
            })
        })
        .collect();

    Ok(json!({ "entries": lines }))
}
