//! RPC handlers for reading/writing structured agent files (AGENTS.md, SOUL.md, etc.).

use std::sync::Arc;

use serde_json::{json, Value};

use super::router::RpcContext;
use super::types::RpcError;

/// Allowlist of agent file names that can be read/written via RPC.
const ALLOWED_AGENT_FILES: &[&str] = &[
    "AGENTS.md",
    "SOUL.md",
    "TOOLS.md",
    "IDENTITY.md",
    "USER.md",
    "HEARTBEAT.md",
    "MEMORY.md",
];

fn validate_filename(name: &str) -> Result<(), RpcError> {
    if !ALLOWED_AGENT_FILES.contains(&name) {
        return Err(RpcError::invalid_request(format!(
            "file '{}' is not in the allowed list: {:?}",
            name, ALLOWED_AGENT_FILES
        )));
    }
    // Belt-and-suspenders: no path separators or null bytes
    if name.contains('/') || name.contains('\\') || name.contains('\0') || name.contains("..") {
        return Err(RpcError::invalid_request("invalid characters in filename"));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// agents.files.list
// ---------------------------------------------------------------------------

pub async fn handle_list(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let agent = params.get("agent").and_then(|v| v.as_str());
    let dir = ctx.state.config.workspace_dir_for_agent(agent);

    let mut files = Vec::new();
    for &name in ALLOWED_AGENT_FILES {
        let path = dir.join(name);
        let (exists, size, modified) = match tokio::fs::metadata(&path).await {
            Ok(meta) => {
                let size = meta.len();
                let modified = meta
                    .modified()
                    .ok()
                    .and_then(|t| {
                        t.duration_since(std::time::UNIX_EPOCH)
                            .ok()
                            .map(|d| d.as_secs())
                    })
                    .unwrap_or(0);
                (true, size, modified)
            }
            Err(_) => (false, 0u64, 0u64),
        };
        files.push(json!({
            "name": name,
            "exists": exists,
            "size": size,
            "modified": modified,
        }));
    }

    Ok(json!({ "files": files }))
}

// ---------------------------------------------------------------------------
// agents.files.get
// ---------------------------------------------------------------------------

pub async fn handle_get(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let agent = params.get("agent").and_then(|v| v.as_str());
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'name' parameter"))?;

    validate_filename(name)?;

    let dir = ctx.state.config.workspace_dir_for_agent(agent);
    let path = dir.join(name);

    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| RpcError::internal(format!("read '{}': {}", name, e)))?;

    let size = content.len() as u64;

    Ok(json!({ "name": name, "content": content, "size": size }))
}

// ---------------------------------------------------------------------------
// agents.files.set
// ---------------------------------------------------------------------------

pub async fn handle_set(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let agent = params.get("agent").and_then(|v| v.as_str());
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'name' parameter"))?;
    let content = params
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'content' parameter"))?;

    validate_filename(name)?;

    let dir = ctx.state.config.workspace_dir_for_agent(agent);

    // Ensure directory exists
    tokio::fs::create_dir_all(&dir)
        .await
        .map_err(|e| RpcError::internal(format!("create dir: {}", e)))?;

    let path = dir.join(name);

    tokio::fs::write(&path, content)
        .await
        .map_err(|e| RpcError::internal(format!("write '{}': {}", name, e)))?;

    let size = content.len() as u64;

    Ok(json!({ "ok": true, "name": name, "size": size }))
}
