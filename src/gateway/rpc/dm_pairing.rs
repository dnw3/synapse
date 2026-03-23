//! RPC handlers for DM pairing management.
//!
//! Methods: dm.pairing.list, dm.pairing.approve, dm.pairing.allowlist, dm.pairing.remove

use std::sync::Arc;

use crate::channels::dm::DmPolicyEnforcer;
use serde_json::{json, Value};

use super::router::RpcContext;
use super::types::RpcError;

/// List pending pairing requests for a channel.
/// Params: { "channel": "lark" }
pub async fn handle_list(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let channel = params["channel"].as_str().unwrap_or("lark");

    let pending = ctx.state.channel.dm_enforcer.list_pending(channel).await;
    let items: Vec<Value> = pending
        .iter()
        .map(|p| {
            json!({
                "code": p.code,
                "sender_id": p.sender_id,
                "channel": p.channel,
                "created_at": p.created_at,
                "ttl_ms": p.ttl_ms,
            })
        })
        .collect();

    Ok(json!({ "pending": items }))
}

/// Approve a pairing code.
/// Params: { "channel": "lark", "code": "DGPG5MDQ" }
pub async fn handle_approve(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let channel = params["channel"]
        .as_str()
        .ok_or_else(|| RpcError::invalid_request("missing 'channel'"))?;
    let code = params["code"]
        .as_str()
        .ok_or_else(|| RpcError::invalid_request("missing 'code'"))?;

    match ctx.state.channel.dm_enforcer.approve_code(channel, code).await {
        Ok(sender_id) => {
            // Notify the user that they've been approved
            ctx.state
                .channel.approve_notifiers
                .notify(channel, &sender_id)
                .await;
            Ok(json!({ "approved": true, "sender_id": sender_id }))
        }
        Err(e) => Ok(json!({ "approved": false, "error": format!("{e:?}") })),
    }
}

/// List the allowlist for a channel.
/// Params: { "channel": "lark" }
pub async fn handle_allowlist(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let channel = params["channel"].as_str().unwrap_or("lark");

    let list = ctx.state.channel.dm_enforcer.get_allowlist(channel);
    Ok(json!({ "allowlist": list }))
}

/// List all channels that have pairing data (pending or allowlist files).
/// Params: {} (none)
pub async fn handle_channels(ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    let channels = ctx.state.channel.dm_enforcer.list_channels();
    Ok(json!({ "channels": channels }))
}

/// Remove a sender from the allowlist.
/// Params: { "channel": "lark", "sender_id": "ou_xxx" }
pub async fn handle_remove(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let channel = params["channel"]
        .as_str()
        .ok_or_else(|| RpcError::invalid_request("missing 'channel'"))?;
    let sender_id = params["sender_id"]
        .as_str()
        .ok_or_else(|| RpcError::invalid_request("missing 'sender_id'"))?;

    let removed = ctx
        .state
        .channel.dm_enforcer
        .remove_from_allowlist(channel, sender_id);
    Ok(json!({ "removed": removed }))
}
