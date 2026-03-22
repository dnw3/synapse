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

/// Load disabled plugin names from persistent state file.
fn load_disabled_plugins() -> Vec<String> {
    let path = dirs::home_dir()
        .unwrap_or_default()
        .join(".synapse/plugins/state.json");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|data| serde_json::from_str::<Value>(&data).ok())
        .and_then(|v| v["disabled"].as_array().cloned())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

// ---------------------------------------------------------------------------
// plugins.list
// ---------------------------------------------------------------------------

/// Return a list of all registered plugins from the runtime PluginRegistry.
///
/// Each entry includes manifest metadata, registration details (tools,
/// interceptors, subscribers, services), and enabled/disabled state.
pub async fn handle_list(ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    let mut plugins: Vec<Value> = Vec::new();
    let disabled = load_disabled_plugins();

    // Collect all data from the registry under a single, brief std::sync::RwLock
    // read guard.  We must NOT hold this guard across .await points.
    struct PluginInfo {
        name: String,
        version: String,
        description: String,
        author: Option<String>,
        license: Option<String>,
        capabilities: Vec<String>,
        slot: Option<String>,
        tools: Vec<String>,
        interceptors: Vec<String>,
        subscribers: Vec<String>,
        service_ids: Vec<String>,
    }

    let plugin_data: Vec<PluginInfo> = {
        let registry = ctx.state.plugin_registry.read().unwrap();
        registry
            .plugins()
            .iter()
            .map(|m| {
                let regs = registry.plugin_registrations(&m.name);
                let caps: Vec<String> = m
                    .capabilities
                    .iter()
                    .map(|c| format!("{:?}", c).to_lowercase())
                    .collect();
                let slot = m.slot.as_ref().map(|s| format!("{:?}", s).to_lowercase());
                PluginInfo {
                    name: m.name.clone(),
                    version: m.version.clone(),
                    description: m.description.clone(),
                    author: m.author.clone(),
                    license: m.license.clone(),
                    capabilities: caps,
                    slot,
                    tools: regs.map(|r| r.tools.clone()).unwrap_or_default(),
                    interceptors: regs.map(|r| r.interceptors.clone()).unwrap_or_default(),
                    subscribers: regs.map(|r| r.subscribers.clone()).unwrap_or_default(),
                    service_ids: regs.map(|r| r.services.clone()).unwrap_or_default(),
                }
            })
            .collect()
    };
    // Lock is dropped here.

    for info in &plugin_data {
        // TODO: Runtime health check requires migrating plugin_registry to
        // tokio::RwLock so we can call async `service.health_check()` without
        // holding a std::sync guard across .await.  For now, report "unknown".
        let health = "unknown";

        let services_info: Vec<Value> = info
            .service_ids
            .iter()
            .map(|id| json!({ "id": id, "status": "unknown" }))
            .collect();

        let enabled = !disabled.contains(&info.name);
        let source = if info.name.starts_with("builtin-") || info.name.starts_with("memory-") {
            "builtin"
        } else {
            "external"
        };

        plugins.push(json!({
            "name": info.name,
            "version": info.version,
            "description": info.description,
            "author": info.author,
            "license": info.license,
            "source": source,
            "enabled": enabled,
            "slot": info.slot,
            "capabilities": info.capabilities,
            "health": health,
            "tools": info.tools,
            "interceptors": info.interceptors,
            "subscribers": info.subscribers,
            "services": services_info,
        }));
    }

    Ok(json!({ "plugins": plugins }))
}

// ---------------------------------------------------------------------------
// plugins.toggle
// ---------------------------------------------------------------------------

/// Toggle a plugin's enabled/disabled state.
///
/// Persists the state to `~/.synapse/plugins/state.json`.
/// The change takes effect after restart.
pub async fn handle_toggle(_ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let name = params["name"]
        .as_str()
        .ok_or_else(|| RpcError::invalid_request("missing 'name'"))?;
    let enabled = params["enabled"]
        .as_bool()
        .ok_or_else(|| RpcError::invalid_request("missing 'enabled'"))?;

    let state_path = dirs::home_dir()
        .unwrap_or_default()
        .join(".synapse/plugins/state.json");

    let mut disabled = load_disabled_plugins();
    if enabled {
        disabled.retain(|d| d != name);
    } else if !disabled.contains(&name.to_string()) {
        disabled.push(name.to_string());
    }

    let state = json!({ "disabled": disabled });
    if let Some(parent) = state_path.parent() {
        std::fs::create_dir_all(parent).ok();
    }
    std::fs::write(&state_path, serde_json::to_string_pretty(&state).unwrap())
        .map_err(|e| RpcError::internal(format!("failed to save state: {e}")))?;

    Ok(json!({
        "ok": true,
        "name": name,
        "enabled": enabled,
        "message": "Takes effect after restart",
    }))
}

// ---------------------------------------------------------------------------
// plugins.service_control
// ---------------------------------------------------------------------------

/// Start or stop a plugin-managed service by ID.
///
/// Currently returns an error because `Service::start()`/`stop()` are async
/// and our `plugin_registry` uses `std::sync::RwLock`.
pub async fn handle_service_control(
    _ctx: Arc<RpcContext>,
    params: Value,
) -> Result<Value, RpcError> {
    let service_id = params["service"]
        .as_str()
        .ok_or_else(|| RpcError::invalid_request("missing 'service'"))?;
    let action = params["action"]
        .as_str()
        .ok_or_else(|| RpcError::invalid_request("missing 'action'"))?;

    // TODO: Requires migrating plugin_registry to tokio::RwLock to hold across
    // async service start/stop calls.
    Err(RpcError::internal(format!(
        "Service control not yet available (requires async registry migration). \
         Action: {action} on {service_id}"
    )))
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
