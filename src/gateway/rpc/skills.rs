//! RPC handlers for skill management.

use std::path::Path;
use std::sync::Arc;

use serde_json::{json, Value};

use super::router::RpcContext;
use super::types::RpcError;

// ---------------------------------------------------------------------------
// skills.status
// ---------------------------------------------------------------------------

pub async fn handle_status(ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    let mut skills = Vec::new();

    let dirs: Vec<(&str, String)> = {
        let mut v = vec![("project", ".claude/skills".to_string())];
        if let Some(home) = dirs::home_dir() {
            v.push((
                "personal",
                home.join(".synapse/skills").to_string_lossy().to_string(),
            ));
            v.push((
                "personal",
                home.join(".claude/skills").to_string_lossy().to_string(),
            ));
        }
        v
    };

    let mut seen_names = std::collections::HashSet::new();

    for (source, dir_path) in dirs {
        if dir_path.is_empty() {
            continue;
        }
        let dir = Path::new(&dir_path);
        if !dir.exists() {
            continue;
        }

        // Scan flat .md files
        if let Ok(mut entries) = tokio::fs::read_dir(dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("md") {
                    let name = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();
                    if !seen_names.insert(name.clone()) {
                        continue;
                    }
                    let enabled = !ctx
                        .state
                        .core
                        .config
                        .skill_overrides
                        .get(&name)
                        .map(|o| !o.enabled)
                        .unwrap_or(false);
                    skills.push(json!({
                        "name": name,
                        "path": path.to_string_lossy(),
                        "source": source,
                        "enabled": enabled,
                    }));
                }
            }
        }

        // Scan subdirectories for SKILL.md
        if let Ok(mut entries) = tokio::fs::read_dir(dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let sub_path = entry.path();
                if !sub_path.is_dir() {
                    continue;
                }
                let skill_md = sub_path.join("SKILL.md");
                if !skill_md.exists() {
                    continue;
                }
                let name = sub_path
                    .file_name()
                    .and_then(|s| s.to_str())
                    .unwrap_or("unknown")
                    .to_string();
                if !seen_names.insert(name.clone()) {
                    continue;
                }
                let enabled = !ctx
                    .state
                    .core
                    .config
                    .skill_overrides
                    .get(&name)
                    .map(|o| !o.enabled)
                    .unwrap_or(false);
                skills.push(json!({
                    "name": name,
                    "path": skill_md.to_string_lossy(),
                    "source": source,
                    "enabled": enabled,
                }));
            }
        }
    }

    Ok(json!(skills))
}

// ---------------------------------------------------------------------------
// skills.bins
// ---------------------------------------------------------------------------

pub async fn handle_bins(_ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    // List required binaries across all skills — placeholder
    Ok(json!([]))
}

// ---------------------------------------------------------------------------
// skills.install
// ---------------------------------------------------------------------------

pub async fn handle_install(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let slug = params
        .get("slug")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'slug' parameter"))?;

    let hub = crate::hub::ClawHubClient::from_config(&ctx.state.core.config);
    crate::hub::install::install_from_hub(&hub, slug, None, false)
        .await
        .map_err(|e| RpcError::internal(format!("install: {}", e)))?;

    Ok(json!({ "ok": true }))
}

// ---------------------------------------------------------------------------
// skills.update
// ---------------------------------------------------------------------------

pub async fn handle_update(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'name' parameter"))?;

    // Re-install = update
    let hub = crate::hub::ClawHubClient::from_config(&ctx.state.core.config);
    crate::hub::install::install_from_hub(&hub, name, None, true)
        .await
        .map_err(|e| RpcError::internal(format!("update: {}", e)))?;

    Ok(json!({ "ok": true }))
}
