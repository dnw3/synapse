//! RPC handlers for agent management.

use std::sync::Arc;

use serde_json::{json, Value};

use super::router::RpcContext;
use super::types::RpcError;

// ---------------------------------------------------------------------------
// Helpers (shared with dashboard.rs)
// ---------------------------------------------------------------------------

fn config_file_path() -> String {
    if std::path::Path::new("synapse.toml").exists() {
        "synapse.toml".to_string()
    } else {
        "synapse.toml.example".to_string()
    }
}

async fn read_config_file() -> Result<(String, String), RpcError> {
    let path = config_file_path();
    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| RpcError::internal(format!("read config: {}", e)))?;
    Ok((path, content))
}

#[allow(clippy::too_many_arguments)]
fn build_agent_route_toml(
    name: &str,
    model: Option<&str>,
    system_prompt: Option<&str>,
    description: Option<&str>,
    pattern: Option<&str>,
    channels: &[String],
    users: &[String],
    priority: Option<u32>,
    workspace: Option<&str>,
) -> toml::Value {
    let mut tbl = toml::map::Map::new();
    tbl.insert("name".to_string(), toml::Value::String(name.to_string()));
    if let Some(m) = model {
        tbl.insert("model".to_string(), toml::Value::String(m.to_string()));
    }
    if let Some(sp) = system_prompt {
        tbl.insert(
            "system_prompt".to_string(),
            toml::Value::String(sp.to_string()),
        );
    }
    if let Some(desc) = description {
        tbl.insert(
            "description".to_string(),
            toml::Value::String(desc.to_string()),
        );
    }
    if let Some(p) = pattern {
        tbl.insert("pattern".to_string(), toml::Value::String(p.to_string()));
    }
    if !channels.is_empty() {
        tbl.insert(
            "channels".to_string(),
            toml::Value::Array(
                channels
                    .iter()
                    .map(|c| toml::Value::String(c.clone()))
                    .collect(),
            ),
        );
    }
    if !users.is_empty() {
        tbl.insert(
            "users".to_string(),
            toml::Value::Array(
                users
                    .iter()
                    .map(|u| toml::Value::String(u.clone()))
                    .collect(),
            ),
        );
    }
    if let Some(pri) = priority {
        tbl.insert("priority".to_string(), toml::Value::Integer(pri as i64));
    }
    if let Some(ws) = workspace {
        tbl.insert("workspace".to_string(), toml::Value::String(ws.to_string()));
    }
    toml::Value::Table(tbl)
}

// ---------------------------------------------------------------------------
// agents.list
// ---------------------------------------------------------------------------

pub async fn handle_list(ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    let mut agents = Vec::new();
    let effective = ctx.state.config.effective_agents();

    // Default agent entry
    agents.push(json!({
        "name": "default",
        "id": "default",
        "model": ctx.state.config.base.model.model,
        "system_prompt": ctx.state.config.base.agent.system_prompt,
        "channels": [],
        "is_default": true,
        "workspace": ctx.state.config.workspace_dir().to_string_lossy(),
        "dm_scope": format!("{:?}", crate::config::DmSessionScope::default()).to_lowercase(),
        "tool_allow": [],
        "tool_deny": [],
    }));

    // Agents from new [agents] config (effective_agents migrates legacy routes)
    for agent_def in &effective.list {
        let is_default_agent = agent_def.id == effective.default;
        let workspace = crate::config::agent_workspace_dir(agent_def);
        agents.push(json!({
            "name": agent_def.id,
            "id": agent_def.id,
            "description": agent_def.description,
            "model": agent_def.model.clone().unwrap_or_else(|| ctx.state.config.base.model.model.clone()),
            "system_prompt": agent_def.system_prompt,
            "is_default": is_default_agent,
            "workspace": workspace.to_string_lossy(),
            "dm_scope": format!("{:?}", agent_def.dm_scope).to_lowercase(),
            "group_session_scope": agent_def.group_session_scope.as_ref().map(|s| format!("{:?}", s).to_lowercase()),
            "tool_allow": agent_def.tool_allow,
            "tool_deny": agent_def.tool_deny,
            "skills_dir": agent_def.skills_dir,
        }));
    }

    Ok(json!(agents))
}

// ---------------------------------------------------------------------------
// agents.create
// ---------------------------------------------------------------------------

pub async fn handle_create(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'name' parameter"))?;

    if name == "default" {
        return Err(RpcError::invalid_request(
            "cannot create agent named 'default'",
        ));
    }

    let (path, content) = read_config_file().await?;
    let mut doc: toml::Value =
        toml::from_str(&content).map_err(|e| RpcError::internal(format!("parse TOML: {}", e)))?;

    // Check for duplicate
    if let Some(toml::Value::Array(arr)) = doc.get("agent_routes") {
        if arr
            .iter()
            .any(|r| r.get("name").and_then(|n| n.as_str()) == Some(name))
        {
            return Err(RpcError::invalid_request(format!(
                "agent '{}' already exists",
                name
            )));
        }
    }

    let model = params.get("model").and_then(|v| v.as_str());
    let system_prompt = params.get("system_prompt").and_then(|v| v.as_str());
    let description = params.get("description").and_then(|v| v.as_str());
    let pattern = params.get("pattern").and_then(|v| v.as_str());
    let channels: Vec<String> = params
        .get("channels")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let users: Vec<String> = params
        .get("users")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let priority = params
        .get("priority")
        .and_then(|v| v.as_u64())
        .map(|p| p as u32);
    let workspace = params.get("workspace").and_then(|v| v.as_str());

    let new_entry = build_agent_route_toml(
        name,
        model,
        system_prompt,
        description,
        pattern,
        &channels,
        &users,
        priority,
        workspace,
    );

    let routes = doc
        .as_table_mut()
        .unwrap()
        .entry("agent_routes")
        .or_insert_with(|| toml::Value::Array(Vec::new()));
    if let toml::Value::Array(arr) = routes {
        arr.push(new_entry);
    }

    let new_content = toml::to_string_pretty(&doc)
        .map_err(|e| RpcError::internal(format!("serialize: {}", e)))?;
    tokio::fs::write(&path, &new_content)
        .await
        .map_err(|e| RpcError::internal(format!("write: {}", e)))?;

    Ok(json!({
        "name": name,
        "model": model.unwrap_or(&ctx.state.config.base.model.model),
        "system_prompt": system_prompt,
        "channels": channels,
        "is_default": false,
        "workspace": ctx.state.config.workspace_dir_for_agent(Some(name)).to_string_lossy(),
    }))
}

// ---------------------------------------------------------------------------
// agents.update
// ---------------------------------------------------------------------------

pub async fn handle_update(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'name' parameter"))?;

    if name == "default" {
        return Err(RpcError::invalid_request(
            "cannot modify the default agent via this method",
        ));
    }

    let (path, content) = read_config_file().await?;
    let mut doc: toml::Value =
        toml::from_str(&content).map_err(|e| RpcError::internal(format!("parse TOML: {}", e)))?;

    let model = params.get("model").and_then(|v| v.as_str());
    let system_prompt = params.get("system_prompt").and_then(|v| v.as_str());
    let description = params.get("description").and_then(|v| v.as_str());
    let pattern = params.get("pattern").and_then(|v| v.as_str());
    let channels: Vec<String> = params
        .get("channels")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let users: Vec<String> = params
        .get("users")
        .and_then(|v| v.as_array())
        .map(|a| {
            a.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    let priority = params
        .get("priority")
        .and_then(|v| v.as_u64())
        .map(|p| p as u32);
    let workspace = params.get("workspace").and_then(|v| v.as_str());

    if let Some(toml::Value::Array(arr)) = doc.get_mut("agent_routes") {
        if let Some(pos) = arr
            .iter()
            .position(|r| r.get("name").and_then(|n| n.as_str()) == Some(name))
        {
            arr[pos] = build_agent_route_toml(
                name,
                model,
                system_prompt,
                description,
                pattern,
                &channels,
                &users,
                priority,
                workspace,
            );
        } else {
            return Err(RpcError::not_found(format!("agent '{}' not found", name)));
        }
    } else {
        return Err(RpcError::not_found("no agent_routes configured"));
    }

    let new_content = toml::to_string_pretty(&doc)
        .map_err(|e| RpcError::internal(format!("serialize: {}", e)))?;
    tokio::fs::write(&path, &new_content)
        .await
        .map_err(|e| RpcError::internal(format!("write: {}", e)))?;

    tracing::info!(agent = %name, "agent updated via RPC");

    Ok(json!({
        "name": name,
        "model": model.unwrap_or(&ctx.state.config.base.model.model),
        "system_prompt": system_prompt,
        "channels": channels,
        "is_default": false,
        "workspace": ctx.state.config.workspace_dir_for_agent(Some(name)).to_string_lossy(),
    }))
}

// ---------------------------------------------------------------------------
// agents.delete
// ---------------------------------------------------------------------------

pub async fn handle_delete(_ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'name' parameter"))?;

    if name == "default" {
        return Err(RpcError::invalid_request("cannot delete the default agent"));
    }

    let (path, content) = read_config_file().await?;
    let mut doc: toml::Value =
        toml::from_str(&content).map_err(|e| RpcError::internal(format!("parse TOML: {}", e)))?;

    if let Some(toml::Value::Array(arr)) = doc.get_mut("agent_routes") {
        let before = arr.len();
        arr.retain(|r| r.get("name").and_then(|n| n.as_str()) != Some(name));
        if arr.len() == before {
            return Err(RpcError::not_found(format!("agent '{}' not found", name)));
        }
    } else {
        return Err(RpcError::not_found(format!("agent '{}' not found", name)));
    }

    let new_content = toml::to_string_pretty(&doc)
        .map_err(|e| RpcError::internal(format!("serialize: {}", e)))?;
    tokio::fs::write(&path, &new_content)
        .await
        .map_err(|e| RpcError::internal(format!("write: {}", e)))?;

    Ok(json!({ "ok": true }))
}
