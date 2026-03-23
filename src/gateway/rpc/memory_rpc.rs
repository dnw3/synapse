//! RPC handlers for memory provider operations.

use std::sync::Arc;

use serde_json::{json, Value};

use super::router::RpcContext;
use super::types::RpcError;

// ---------------------------------------------------------------------------
// memory.search
// ---------------------------------------------------------------------------

pub async fn handle_search(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let query = params["query"].as_str().unwrap_or("");
    let session_key = params["session_key"].as_str();
    let limit = params["limit"].as_u64().unwrap_or(6) as usize;

    let results = ctx
        .state
        .agent
        .memory_provider
        .search(query, session_key, limit)
        .await
        .map_err(|e| RpcError::internal(format!("memory search failed: {}", e)))?;

    Ok(json!(results))
}

// ---------------------------------------------------------------------------
// memory.add_resource
// ---------------------------------------------------------------------------

pub async fn handle_add_resource(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let uri = params["uri"].as_str().unwrap_or("");

    ctx.state
        .agent
        .memory_provider
        .add_resource(uri)
        .await
        .map_err(|e| RpcError::internal(format!("add resource failed: {}", e)))?;

    Ok(json!({"ok": true}))
}
