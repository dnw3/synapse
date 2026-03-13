//! RPC handlers for the skill store (ClawHub).

use std::sync::Arc;

use serde_json::{json, Value};

use super::router::RpcContext;
use super::types::RpcError;

// ---------------------------------------------------------------------------
// store.search
// ---------------------------------------------------------------------------

pub async fn handle_search(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let query = params
        .get("query")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'query' parameter"))?;
    let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;

    let hub = crate::hub::ClawHubClient::from_config(&ctx.state.config);
    let results = hub
        .search(query, limit)
        .await
        .map_err(|e| RpcError::internal(format!("store search: {}", e)))?;

    Ok(json!({ "results": results, "source": "clawhub" }))
}

// ---------------------------------------------------------------------------
// store.list
// ---------------------------------------------------------------------------

pub async fn handle_list(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(20) as usize;
    let sort = params.get("sort").and_then(|v| v.as_str());
    let cursor = params.get("cursor").and_then(|v| v.as_str());

    let hub = crate::hub::ClawHubClient::from_config(&ctx.state.config);
    let items = hub
        .list(limit, sort, cursor)
        .await
        .map_err(|e| RpcError::internal(format!("store list: {}", e)))?;

    Ok(json!({ "items": items, "source": "clawhub" }))
}

// ---------------------------------------------------------------------------
// store.detail
// ---------------------------------------------------------------------------

pub async fn handle_detail(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let slug = params
        .get("slug")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'slug' parameter"))?;

    let hub = crate::hub::ClawHubClient::from_config(&ctx.state.config);
    let detail = hub
        .detail(slug)
        .await
        .map_err(|e| RpcError::internal(format!("store detail: {}", e)))?;

    Ok(detail)
}

// ---------------------------------------------------------------------------
// store.install
// ---------------------------------------------------------------------------

pub async fn handle_install(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let slug = params
        .get("slug")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'slug' parameter"))?;
    let version = params.get("version").and_then(|v| v.as_str());

    let hub = crate::hub::ClawHubClient::from_config(&ctx.state.config);
    crate::hub::install::install_from_hub(&hub, slug, version, false)
        .await
        .map_err(|e| RpcError::internal(format!("install: {}", e)))?;

    Ok(json!({ "ok": true }))
}

// ---------------------------------------------------------------------------
// store.status
// ---------------------------------------------------------------------------

pub async fn handle_status(ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    let hub = crate::hub::ClawHubClient::from_config(&ctx.state.config);
    let configured = hub.is_configured();
    let lock = crate::hub::install::read_lock_file();
    let installed_count = lock.skills.len();
    let installed: Vec<String> = lock.skills.keys().cloned().collect();

    Ok(json!({
        "configured": configured,
        "installedCount": installed_count,
        "installed": installed,
        "source": "clawhub",
    }))
}
