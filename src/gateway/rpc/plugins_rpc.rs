//! RPC handlers for plugin management.

use std::path::PathBuf;
use std::sync::Arc;

use serde_json::{json, Value};

use super::router::RpcContext;
use super::types::RpcError;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn workspace_plugins_dir() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".synapse")
        .join("plugins")
}

fn global_plugins_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("~"))
        .join(".synapse")
        .join("plugins")
}

// ---------------------------------------------------------------------------
// plugins.list
// ---------------------------------------------------------------------------

/// Return a list of all installed plugins: builtin manifests + external
/// (workspace + global) discovered from filesystem.
pub async fn handle_list(_ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    let mut plugins: Vec<Value> = Vec::new();

    // 1. Builtin plugins (hardcoded, always present)
    let version = env!("CARGO_PKG_VERSION");
    for (name, description) in [
        ("builtin-tracing", "Agent tracing and latency measurement"),
        ("builtin-thinking", "Extended thinking configuration"),
        (
            "builtin-loop-detection",
            "Detect and break agent execution loops",
        ),
    ] {
        plugins.push(json!({
            "name": name,
            "version": version,
            "description": description,
            "author": "synapse",
            "source": "builtin",
            "enabled": true,
        }));
    }

    // 2. External plugins (workspace + global)
    let dirs_with_scope: Vec<(PathBuf, &str)> = vec![
        (workspace_plugins_dir(), "workspace"),
        (global_plugins_dir(), "global"),
    ];

    for (dir, scope) in dirs_with_scope {
        if !dir.exists() {
            continue;
        }
        let mut entries = match tokio::fs::read_dir(&dir).await {
            Ok(e) => e,
            Err(err) => {
                tracing::warn!(dir = %dir.display(), error = %err, "plugins.list: failed to read dir");
                continue;
            }
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let sub = entry.path();
            if !sub.is_dir() {
                continue;
            }
            let manifest_path = sub.join("plugin.toml");
            if !manifest_path.exists() {
                continue;
            }
            let contents = match tokio::fs::read_to_string(&manifest_path).await {
                Ok(s) => s,
                Err(err) => {
                    tracing::warn!(path = %manifest_path.display(), error = %err, "plugins.list: failed to read manifest");
                    continue;
                }
            };
            // Parse just the [plugin] section we need.
            #[derive(serde::Deserialize, Default)]
            struct PluginSection {
                #[serde(default)]
                name: String,
                #[serde(default)]
                version: String,
                #[serde(default)]
                description: String,
                #[serde(default)]
                author: Option<String>,
            }
            #[derive(serde::Deserialize)]
            struct Manifest {
                #[serde(default)]
                plugin: PluginSection,
            }
            let manifest: Manifest = match toml::from_str(&contents) {
                Ok(m) => m,
                Err(err) => {
                    tracing::warn!(path = %manifest_path.display(), error = %err, "plugins.list: invalid manifest");
                    continue;
                }
            };
            let name = if manifest.plugin.name.is_empty() {
                sub.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("unknown")
                    .to_string()
            } else {
                manifest.plugin.name
            };
            plugins.push(json!({
                "name": name,
                "version": manifest.plugin.version,
                "description": manifest.plugin.description,
                "author": manifest.plugin.author,
                "source": scope,
                "enabled": true,
            }));
        }
    }

    Ok(json!(plugins))
}

// ---------------------------------------------------------------------------
// plugins.install
// ---------------------------------------------------------------------------

/// Install a plugin by name (and optional version).
///
/// If `name` is an existing local path with `plugin.toml`, copies it into the
/// workspace plugin directory. Otherwise creates a placeholder entry in the
/// global plugins directory (registry download not yet implemented).
pub async fn handle_install(_ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'name' parameter"))?
        .to_string();

    let _version = params.get("version").and_then(|v| v.as_str());

    // Check if it's an existing local directory
    let source_path = std::path::Path::new(&name);
    if source_path.exists() && source_path.is_dir() {
        let manifest_path = source_path.join("plugin.toml");
        if !manifest_path.exists() {
            return Err(RpcError::invalid_request(format!(
                "no plugin.toml found in '{}'. A plugin directory must contain a plugin.toml manifest.",
                source_path.display()
            )));
        }

        #[derive(serde::Deserialize)]
        struct LocalManifest {
            name: String,
            version: String,
        }
        let contents = tokio::fs::read_to_string(&manifest_path)
            .await
            .map_err(|e| RpcError::internal(format!("failed to read manifest: {}", e)))?;
        let manifest: LocalManifest = toml::from_str(&contents)
            .map_err(|e| RpcError::invalid_request(format!("invalid plugin.toml: {}", e)))?;

        let dest = workspace_plugins_dir().join(&manifest.name);
        if dest.exists() {
            return Err(RpcError::invalid_request(format!(
                "plugin '{}' is already installed",
                manifest.name
            )));
        }

        tokio::fs::create_dir_all(&dest)
            .await
            .map_err(|e| RpcError::internal(format!("failed to create directory: {}", e)))?;

        copy_dir_recursive(source_path, &dest)
            .await
            .map_err(|e| RpcError::internal(format!("failed to copy plugin: {}", e)))?;

        return Ok(json!({
            "ok": true,
            "name": manifest.name,
            "version": manifest.version,
            "message": "Plugin installed from local path. Restart Synapse to activate.",
        }));
    }

    // Name-based install — create placeholder (registry not yet implemented)
    let global_dir = global_plugins_dir();
    tokio::fs::create_dir_all(&global_dir)
        .await
        .map_err(|e| RpcError::internal(format!("failed to create plugins directory: {}", e)))?;

    let dest = global_dir.join(&name);
    if dest.exists() {
        return Err(RpcError::invalid_request(format!(
            "plugin '{}' is already installed",
            name
        )));
    }

    tokio::fs::create_dir_all(&dest)
        .await
        .map_err(|e| RpcError::internal(format!("failed to create plugin directory: {}", e)))?;

    let placeholder = format!(
        r#"[plugin]
name = "{name}"
version = "unknown"
description = "Installed via registry (placeholder — registry download not yet implemented)"

[runtime]
command = ""
args = []
transport = "stdio"

[capabilities]
tools = false
"#,
        name = name
    );
    tokio::fs::write(dest.join("plugin.toml"), &placeholder)
        .await
        .map_err(|e| RpcError::internal(format!("failed to write manifest: {}", e)))?;

    Ok(json!({
        "ok": true,
        "name": name,
        "version": "unknown",
        "message": "Plugin placeholder created. Registry download not yet implemented.",
    }))
}

// ---------------------------------------------------------------------------
// plugins.remove
// ---------------------------------------------------------------------------

/// Remove a plugin by name. Checks workspace directory first, then global.
pub async fn handle_remove(_ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'name' parameter"))?
        .to_string();

    let workspace_plugin = workspace_plugins_dir().join(&name);
    let global_plugin = global_plugins_dir().join(&name);

    let plugin_dir = if workspace_plugin.exists() {
        workspace_plugin
    } else if global_plugin.exists() {
        global_plugin
    } else {
        return Err(RpcError::not_found(format!("plugin '{}' not found", name)));
    };

    tokio::fs::remove_dir_all(&plugin_dir)
        .await
        .map_err(|e| RpcError::internal(format!("failed to remove plugin directory: {}", e)))?;

    Ok(json!({
        "ok": true,
        "name": name,
        "message": format!("Plugin '{}' removed successfully.", name),
    }))
}

// ---------------------------------------------------------------------------
// plugins.marketplace
// ---------------------------------------------------------------------------

/// Search the plugin marketplace / registry.
///
/// Registry is not yet configured; returns a placeholder response.
pub async fn handle_marketplace(_ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    Ok(json!({
        "ok": false,
        "message": "Registry not configured",
        "results": [],
    }))
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

/// Recursively copy `src` directory into `dst`.
async fn copy_dir_recursive(
    src: &std::path::Path,
    dst: &std::path::Path,
) -> Result<(), std::io::Error> {
    let mut stack = vec![(src.to_path_buf(), dst.to_path_buf())];
    while let Some((from, to)) = stack.pop() {
        tokio::fs::create_dir_all(&to).await?;
        let mut entries = tokio::fs::read_dir(&from).await?;
        while let Some(entry) = entries.next_entry().await? {
            let src_path = entry.path();
            let dst_path = to.join(entry.file_name());
            if src_path.is_dir() {
                stack.push((src_path, dst_path));
            } else {
                tokio::fs::copy(&src_path, &dst_path).await?;
            }
        }
    }
    Ok(())
}
