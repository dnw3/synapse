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

// ---------------------------------------------------------------------------
// gateway.identity.get
// ---------------------------------------------------------------------------

pub async fn handle_gateway_identity(
    ctx: Arc<RpcContext>,
    _params: Value,
) -> Result<Value, RpcError> {
    // Persistent device ID stored in data/device_id
    let device_id_path = std::path::Path::new("data/device_id");
    let device_id = if device_id_path.exists() {
        tokio::fs::read_to_string(device_id_path)
            .await
            .unwrap_or_else(|_| uuid::Uuid::new_v4().to_string())
    } else {
        let id = uuid::Uuid::new_v4().to_string();
        let _ = tokio::fs::create_dir_all("data").await;
        let _ = tokio::fs::write(device_id_path, &id).await;
        id
    };

    Ok(json!({
        "device_id": device_id.trim(),
        "version": env!("CARGO_PKG_VERSION"),
        "uptime_secs": ctx.state.started_at.elapsed().as_secs(),
    }))
}
