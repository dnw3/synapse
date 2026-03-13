//! RPC handlers for workspace file management.

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};

use super::router::RpcContext;
use super::types::RpcError;

fn sanitize_filename(filename: &str) -> Result<(), RpcError> {
    if filename.is_empty() || filename.len() > 64 {
        return Err(RpcError::invalid_request(
            "filename must be 1-64 characters",
        ));
    }
    if !filename.ends_with(".md") {
        return Err(RpcError::invalid_request("filename must end with .md"));
    }
    if filename.contains("..")
        || filename.contains('/')
        || filename.contains('\\')
        || filename.contains('\0')
    {
        return Err(RpcError::invalid_request("invalid characters in filename"));
    }
    if !filename
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    {
        return Err(RpcError::invalid_request(
            "filename may only contain [a-zA-Z0-9._-]",
        ));
    }
    Ok(())
}

fn workspace_dir(config: &crate::config::SynapseConfig, agent: Option<&str>) -> PathBuf {
    config.workspace_dir_for_agent(agent)
}

// ---------------------------------------------------------------------------
// workspace.list
// ---------------------------------------------------------------------------

pub async fn handle_list(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    use crate::agent::templates::WORKSPACE_TEMPLATES;

    let agent = params.get("agent").and_then(|v| v.as_str());
    let cwd = workspace_dir(&ctx.state.config, agent);
    let mut entries = Vec::new();

    for tmpl in WORKSPACE_TEMPLATES {
        let path = cwd.join(tmpl.filename);
        let (exists, size_bytes, modified, preview) = if path.exists() {
            let meta = tokio::fs::metadata(&path).await.ok();
            let size = meta.as_ref().map(|m| m.len());
            let mod_time = meta.as_ref().and_then(|m| m.modified().ok()).map(|t| {
                let dur = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
                chrono::DateTime::from_timestamp(dur.as_secs() as i64, dur.subsec_nanos())
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_default()
            });
            let preview = tokio::fs::read_to_string(&path).await.ok().map(|c| {
                let trimmed = c.trim();
                if trimmed.len() > 200 {
                    format!("{}...", &trimmed[..200])
                } else {
                    trimmed.to_string()
                }
            });
            (true, size, mod_time, preview)
        } else {
            (false, None, None, None)
        };

        entries.push(json!({
            "filename": tmpl.filename,
            "description": tmpl.description,
            "category": tmpl.category,
            "icon": tmpl.icon,
            "exists": exists,
            "size_bytes": size_bytes,
            "modified": modified,
            "preview": preview,
            "is_template": true,
        }));
    }

    // Custom .md files
    if let Ok(mut dir) = tokio::fs::read_dir(&cwd).await {
        while let Ok(Some(entry)) = dir.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.ends_with(".md") {
                continue;
            }
            if WORKSPACE_TEMPLATES.iter().any(|t| t.filename == name) {
                continue;
            }
            if name == "README.md" || name == "CHANGELOG.md" || name == "LICENSE.md" {
                continue;
            }
            let path = cwd.join(&name);
            let meta = tokio::fs::metadata(&path).await.ok();
            let size = meta.as_ref().map(|m| m.len());
            let mod_time = meta.as_ref().and_then(|m| m.modified().ok()).map(|t| {
                let dur = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
                chrono::DateTime::from_timestamp(dur.as_secs() as i64, dur.subsec_nanos())
                    .map(|dt| dt.to_rfc3339())
                    .unwrap_or_default()
            });
            let preview = tokio::fs::read_to_string(&path).await.ok().map(|c| {
                let trimmed = c.trim();
                if trimmed.len() > 200 {
                    format!("{}...", &trimmed[..200])
                } else {
                    trimmed.to_string()
                }
            });
            entries.push(json!({
                "filename": name,
                "description": "Custom workspace file",
                "category": "custom",
                "icon": "file-text",
                "exists": true,
                "size_bytes": size,
                "modified": mod_time,
                "preview": preview,
                "is_template": false,
            }));
        }
    }

    Ok(json!(entries))
}

// ---------------------------------------------------------------------------
// workspace.get
// ---------------------------------------------------------------------------

pub async fn handle_get(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let filename = params
        .get("filename")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'filename' parameter"))?;

    sanitize_filename(filename)?;

    let agent = params.get("agent").and_then(|v| v.as_str());
    let path = workspace_dir(&ctx.state.config, agent).join(filename);
    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|_| RpcError::not_found(format!("file '{}' not found", filename)))?;

    let is_template = crate::agent::templates::find_template(filename).is_some();

    Ok(json!({
        "filename": filename,
        "content": content,
        "is_template": is_template,
    }))
}

// ---------------------------------------------------------------------------
// workspace.set
// ---------------------------------------------------------------------------

pub async fn handle_set(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let filename = params
        .get("filename")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'filename' parameter"))?;
    let content = params
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'content' parameter"))?;

    sanitize_filename(filename)?;

    let agent = params.get("agent").and_then(|v| v.as_str());
    let path = workspace_dir(&ctx.state.config, agent).join(filename);
    if !path.exists() {
        return Err(RpcError::not_found(format!(
            "file '{}' not found -- use workspace.create",
            filename
        )));
    }
    tokio::fs::write(&path, content)
        .await
        .map_err(|e| RpcError::internal(format!("write: {}", e)))?;

    tracing::info!(file = %filename, "workspace file saved via RPC");

    Ok(json!({ "ok": true }))
}

// ---------------------------------------------------------------------------
// workspace.create
// ---------------------------------------------------------------------------

pub async fn handle_create(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let filename = params
        .get("filename")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'filename' parameter"))?;
    let content = params
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'content' parameter"))?;

    sanitize_filename(filename)?;

    let agent = params.get("agent").and_then(|v| v.as_str());
    let path = workspace_dir(&ctx.state.config, agent).join(filename);
    if path.exists() {
        return Err(RpcError::invalid_request(format!(
            "file '{}' already exists -- use workspace.set to update",
            filename
        )));
    }
    tokio::fs::write(&path, content)
        .await
        .map_err(|e| RpcError::internal(format!("write: {}", e)))?;

    Ok(json!({ "ok": true }))
}

// ---------------------------------------------------------------------------
// workspace.delete
// ---------------------------------------------------------------------------

pub async fn handle_delete(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let filename = params
        .get("filename")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'filename' parameter"))?;

    sanitize_filename(filename)?;

    let agent = params.get("agent").and_then(|v| v.as_str());
    let path = workspace_dir(&ctx.state.config, agent).join(filename);
    if !path.exists() {
        return Err(RpcError::not_found(format!(
            "file '{}' not found",
            filename
        )));
    }
    tokio::fs::remove_file(&path)
        .await
        .map_err(|e| RpcError::internal(format!("delete: {}", e)))?;

    Ok(json!({ "ok": true }))
}

// ---------------------------------------------------------------------------
// workspace.reset
// ---------------------------------------------------------------------------

pub async fn handle_reset(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let filename = params
        .get("filename")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'filename' parameter"))?;

    sanitize_filename(filename)?;

    let default = crate::agent::workspace::default_content_for(filename)
        .ok_or_else(|| RpcError::not_found(format!("no default template for '{}'", filename)))?;

    let agent = params.get("agent").and_then(|v| v.as_str());
    let path = workspace_dir(&ctx.state.config, agent).join(filename);
    tokio::fs::write(&path, default)
        .await
        .map_err(|e| RpcError::internal(format!("write: {}", e)))?;

    Ok(json!({ "ok": true }))
}
