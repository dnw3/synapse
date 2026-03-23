//! RPC handlers for binding management.
//!
//! Methods: bindings.list

use std::sync::Arc;

use serde_json::{json, Value};

use super::router::RpcContext;
use super::types::RpcError;

/// List all effective bindings (new format + migrated legacy routes).
pub async fn handle_list(ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    let bindings = ctx.state.core.config.effective_bindings();
    let items: Vec<Value> = bindings
        .iter()
        .map(|b| {
            json!({
                "agent": b.agent,
                "channel": b.channel,
                "account_id": b.account_id,
                "peer": b.peer.as_ref().map(|p| json!({
                    "kind": format!("{:?}", p.kind).to_lowercase(),
                    "id": p.id,
                })),
                "guild_id": b.guild_id,
                "team_id": b.team_id,
                "roles": b.roles,
                "comment": b.comment,
            })
        })
        .collect();
    Ok(json!({ "bindings": items }))
}
