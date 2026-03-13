//! RPC handlers for agent identity.

use std::sync::Arc;

use serde_json::{json, Value};

use super::router::RpcContext;
use super::types::RpcError;

// ---------------------------------------------------------------------------
// agent.identity.get
// ---------------------------------------------------------------------------

pub async fn handle_get(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let agent = params.get("agent").and_then(|v| v.as_str());
    let path = ctx
        .state
        .config
        .workspace_dir_for_agent(agent)
        .join("IDENTITY.md");

    let info = match tokio::fs::read_to_string(&path).await {
        Ok(content) => crate::agent::workspace::parse_identity(&content),
        Err(_) => crate::agent::workspace::IdentityInfo::default(),
    };

    Ok(json!({
        "name": info.name,
        "emoji": info.emoji,
        "avatar_url": info.avatar_url,
        "theme_color": info.theme_color,
    }))
}
