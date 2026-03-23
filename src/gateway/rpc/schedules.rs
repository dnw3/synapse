//! RPC handlers for schedule (cron) management.

use std::path::Path;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use super::router::RpcContext;
use super::types::RpcError;

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

fn build_schedule_toml(
    name: &str,
    prompt: &str,
    cron: Option<&str>,
    interval_secs: Option<u64>,
    enabled: bool,
    description: Option<&str>,
) -> toml::Value {
    let mut tbl = toml::map::Map::new();
    tbl.insert("name".to_string(), toml::Value::String(name.to_string()));
    tbl.insert(
        "prompt".to_string(),
        toml::Value::String(prompt.to_string()),
    );
    if let Some(c) = cron {
        tbl.insert("cron".to_string(), toml::Value::String(c.to_string()));
    }
    if let Some(i) = interval_secs {
        tbl.insert("interval_secs".to_string(), toml::Value::Integer(i as i64));
    }
    tbl.insert("enabled".to_string(), toml::Value::Boolean(enabled));
    if let Some(d) = description {
        tbl.insert(
            "description".to_string(),
            toml::Value::String(d.to_string()),
        );
    }
    toml::Value::Table(tbl)
}

// Schedule run history
const SCHEDULE_RUNS_FILE: &str = "log/schedule_runs.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScheduleRunEntry {
    id: String,
    schedule_name: String,
    started_at: String,
    finished_at: Option<String>,
    status: String,
    result: Option<String>,
    error: Option<String>,
}

async fn read_schedule_runs() -> Vec<ScheduleRunEntry> {
    match tokio::fs::read_to_string(SCHEDULE_RUNS_FILE).await {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => Vec::new(),
    }
}

async fn append_schedule_run(entry: ScheduleRunEntry) {
    let mut runs = read_schedule_runs().await;
    runs.push(entry);
    if let Some(parent) = Path::new(SCHEDULE_RUNS_FILE).parent() {
        let _ = tokio::fs::create_dir_all(parent).await;
    }
    let _ = tokio::fs::write(
        SCHEDULE_RUNS_FILE,
        serde_json::to_string_pretty(&runs).unwrap_or_default(),
    )
    .await;
}

// ---------------------------------------------------------------------------
// cron.list
// ---------------------------------------------------------------------------

pub async fn handle_list(ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    let schedules: Vec<Value> = ctx
        .state
        .core
        .config
        .schedules
        .as_ref()
        .map(|entries| {
            entries
                .iter()
                .map(|e| {
                    json!({
                        "name": e.name,
                        "prompt": e.prompt,
                        "cron": e.cron,
                        "interval_secs": e.interval_secs,
                        "enabled": e.enabled,
                        "description": e.description,
                    })
                })
                .collect()
        })
        .unwrap_or_default();

    Ok(json!(schedules))
}

// ---------------------------------------------------------------------------
// cron.add
// ---------------------------------------------------------------------------

pub async fn handle_add(_ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'name' parameter"))?;
    let prompt = params
        .get("prompt")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'prompt' parameter"))?;
    let cron = params.get("cron").and_then(|v| v.as_str());
    let interval_secs = params.get("interval_secs").and_then(|v| v.as_u64());
    let enabled = params
        .get("enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let description = params.get("description").and_then(|v| v.as_str());

    let (path, content) = read_config_file().await?;
    let mut doc: toml::Value =
        toml::from_str(&content).map_err(|e| RpcError::internal(format!("parse TOML: {}", e)))?;

    let new_entry = build_schedule_toml(name, prompt, cron, interval_secs, enabled, description);

    let schedules = doc
        .as_table_mut()
        .unwrap()
        .entry("schedule")
        .or_insert_with(|| toml::Value::Array(Vec::new()));
    if let toml::Value::Array(arr) = schedules {
        arr.push(new_entry);
    }

    let new_content = toml::to_string_pretty(&doc)
        .map_err(|e| RpcError::internal(format!("serialize: {}", e)))?;
    tokio::fs::write(&path, &new_content)
        .await
        .map_err(|e| RpcError::internal(format!("write: {}", e)))?;

    tracing::info!("schedule created via RPC");

    Ok(json!({
        "name": name,
        "prompt": prompt,
        "cron": cron,
        "interval_secs": interval_secs,
        "enabled": enabled,
        "description": description,
    }))
}

// ---------------------------------------------------------------------------
// cron.update
// ---------------------------------------------------------------------------

pub async fn handle_update(_ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'name' parameter"))?;
    let prompt = params
        .get("prompt")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'prompt' parameter"))?;
    let cron = params.get("cron").and_then(|v| v.as_str());
    let interval_secs = params.get("interval_secs").and_then(|v| v.as_u64());
    let enabled = params
        .get("enabled")
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let description = params.get("description").and_then(|v| v.as_str());

    let (path, content) = read_config_file().await?;
    let mut doc: toml::Value =
        toml::from_str(&content).map_err(|e| RpcError::internal(format!("parse TOML: {}", e)))?;

    if let Some(toml::Value::Array(arr)) = doc.get_mut("schedule") {
        if let Some(pos) = arr
            .iter()
            .position(|s| s.get("name").and_then(|n| n.as_str()) == Some(name))
        {
            arr[pos] = build_schedule_toml(name, prompt, cron, interval_secs, enabled, description);
        } else {
            return Err(RpcError::not_found(format!(
                "schedule '{}' not found",
                name
            )));
        }
    } else {
        return Err(RpcError::not_found("no schedules configured"));
    }

    let new_content = toml::to_string_pretty(&doc)
        .map_err(|e| RpcError::internal(format!("serialize: {}", e)))?;
    tokio::fs::write(&path, &new_content)
        .await
        .map_err(|e| RpcError::internal(format!("write: {}", e)))?;

    Ok(json!({
        "name": name,
        "prompt": prompt,
        "cron": cron,
        "interval_secs": interval_secs,
        "enabled": enabled,
        "description": description,
    }))
}

// ---------------------------------------------------------------------------
// cron.remove
// ---------------------------------------------------------------------------

pub async fn handle_remove(_ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'name' parameter"))?;

    let (path, content) = read_config_file().await?;
    let mut doc: toml::Value =
        toml::from_str(&content).map_err(|e| RpcError::internal(format!("parse TOML: {}", e)))?;

    if let Some(toml::Value::Array(arr)) = doc.get_mut("schedule") {
        let before = arr.len();
        arr.retain(|s| s.get("name").and_then(|n| n.as_str()) != Some(name));
        if arr.len() == before {
            return Err(RpcError::not_found(format!(
                "schedule '{}' not found",
                name
            )));
        }
    }

    let new_content = toml::to_string_pretty(&doc)
        .map_err(|e| RpcError::internal(format!("serialize: {}", e)))?;
    tokio::fs::write(&path, &new_content)
        .await
        .map_err(|e| RpcError::internal(format!("write: {}", e)))?;

    tracing::info!("schedule deleted via RPC");

    Ok(json!({ "ok": true }))
}

// ---------------------------------------------------------------------------
// cron.run
// ---------------------------------------------------------------------------

pub async fn handle_run(_ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'name' parameter"))?;

    let started_at = chrono::Utc::now().to_rfc3339();
    let id = format!("run-{}-{}", name, chrono::Utc::now().timestamp_millis());
    let finished_at = chrono::Utc::now().to_rfc3339();

    append_schedule_run(ScheduleRunEntry {
        id: id.clone(),
        schedule_name: name.to_string(),
        started_at,
        finished_at: Some(finished_at),
        status: "success".to_string(),
        result: Some("Triggered via RPC".to_string()),
        error: None,
    })
    .await;

    Ok(json!({ "ok": true, "run_id": id }))
}

// ---------------------------------------------------------------------------
// cron.runs
// ---------------------------------------------------------------------------

pub async fn handle_runs(_ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'name' parameter"))?;

    let all_runs = read_schedule_runs().await;
    let runs: Vec<Value> = all_runs
        .into_iter()
        .filter(|r| r.schedule_name == name)
        .rev()
        .take(50)
        .map(|r| serde_json::to_value(r).unwrap_or_default())
        .collect();

    Ok(json!(runs))
}

// ---------------------------------------------------------------------------
// cron.status
// ---------------------------------------------------------------------------

pub async fn handle_status_toggle(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'name' parameter"))?;

    // Find the schedule in config to report current enabled state
    let enabled = ctx
        .state
        .core
        .config
        .schedules
        .as_ref()
        .and_then(|entries| entries.iter().find(|e| e.name == name))
        .map(|e| e.enabled)
        .unwrap_or(false);

    Ok(json!({
        "name": name,
        "enabled": enabled,
    }))
}
