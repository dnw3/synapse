//! RPC handlers for broadcast group management.
//!
//! Methods: broadcasts.list

use std::sync::Arc;

use serde_json::{json, Value};

use super::router::RpcContext;
use super::types::RpcError;

/// List all configured broadcast groups.
pub async fn handle_list(ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    let groups: Vec<Value> = ctx
        .state
        .core
        .config
        .broadcasts
        .iter()
        .map(|g| {
            json!({
                "name": g.name,
                "description": g.description,
                "channel": g.channel,
                "peer_id": g.peer_id,
                "agents": g.agents,
                "strategy": format!("{:?}", g.strategy).to_lowercase(),
                "timeout_secs": g.timeout_secs,
            })
        })
        .collect();
    Ok(json!({ "broadcasts": groups }))
}
