use std::path::Path;

use axum::extract::{self, State};
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::{delete, get, post, put};
use axum::Router;
use serde::{Deserialize, Serialize};

use super::{read_config_file, OkResponse, ToggleResponse};
use crate::gateway::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/dashboard/schedules", get(get_schedules))
        .route("/dashboard/schedules", post(create_schedule))
        .route("/dashboard/schedules/{name}", put(update_schedule))
        .route("/dashboard/schedules/{name}", delete(delete_schedule))
        .route(
            "/dashboard/schedules/{name}/trigger",
            post(trigger_schedule),
        )
        .route("/dashboard/schedules/{name}/toggle", post(toggle_schedule))
        .route("/dashboard/schedules/{name}/runs", get(get_schedule_runs))
}

// ---------------------------------------------------------------------------
// GET /api/dashboard/schedules
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct ScheduleResponseItem {
    name: String,
    prompt: String,
    cron: Option<String>,
    interval_secs: Option<u64>,
    enabled: bool,
    description: Option<String>,
}

async fn get_schedules(State(state): State<AppState>) -> Json<Vec<ScheduleResponseItem>> {
    let schedules = state
        .core
        .config
        .schedules
        .as_ref()
        .map(|entries| {
            entries
                .iter()
                .map(|e| ScheduleResponseItem {
                    name: e.name.clone(),
                    prompt: e.prompt.clone(),
                    cron: e.cron.clone(),
                    interval_secs: e.interval_secs,
                    enabled: e.enabled,
                    description: e.description.clone(),
                })
                .collect()
        })
        .unwrap_or_default();

    Json(schedules)
}

// ---------------------------------------------------------------------------
// POST /api/dashboard/schedules
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct CreateScheduleRequest {
    name: String,
    prompt: String,
    cron: Option<String>,
    interval_secs: Option<u64>,
    #[serde(default = "default_true_fn")]
    enabled: bool,
    description: Option<String>,
}

fn default_true_fn() -> bool {
    true
}

async fn create_schedule(
    State(_state): State<AppState>,
    Json(body): Json<CreateScheduleRequest>,
) -> Result<Json<ScheduleResponseItem>, (StatusCode, String)> {
    let (path, content) = read_config_file().await?;
    let mut doc: toml::Value = toml::from_str(&content).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("parse TOML: {}", e),
        )
    })?;

    let new_entry = build_schedule_toml(
        &body.name,
        &body.prompt,
        &body.cron,
        &body.interval_secs,
        body.enabled,
        &body.description,
    );

    let schedules = doc
        .as_table_mut()
        .unwrap()
        .entry("schedule")
        .or_insert_with(|| toml::Value::Array(Vec::new()));
    if let toml::Value::Array(arr) = schedules {
        arr.push(new_entry);
    }

    let new_content = toml::to_string_pretty(&doc).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("serialize: {}", e),
        )
    })?;
    tokio::fs::write(&path, &new_content)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("write: {}", e)))?;

    tracing::info!("schedule created");

    Ok(Json(ScheduleResponseItem {
        name: body.name,
        prompt: body.prompt,
        cron: body.cron,
        interval_secs: body.interval_secs,
        enabled: body.enabled,
        description: body.description,
    }))
}

// ---------------------------------------------------------------------------
// PUT /api/dashboard/schedules/{name}
// ---------------------------------------------------------------------------

async fn update_schedule(
    State(_state): State<AppState>,
    extract::Path(name): extract::Path<String>,
    Json(body): Json<CreateScheduleRequest>,
) -> Result<Json<ScheduleResponseItem>, (StatusCode, String)> {
    let (path, content) = read_config_file().await?;
    let mut doc: toml::Value = toml::from_str(&content).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("parse TOML: {}", e),
        )
    })?;

    if let Some(toml::Value::Array(arr)) = doc.get_mut("schedule") {
        if let Some(pos) = arr
            .iter()
            .position(|s| s.get("name").and_then(|n| n.as_str()) == Some(&name))
        {
            arr[pos] = build_schedule_toml(
                &body.name,
                &body.prompt,
                &body.cron,
                &body.interval_secs,
                body.enabled,
                &body.description,
            );
        } else {
            return Err((
                StatusCode::NOT_FOUND,
                format!("schedule '{}' not found", name),
            ));
        }
    } else {
        return Err((StatusCode::NOT_FOUND, "no schedules configured".to_string()));
    }

    let new_content = toml::to_string_pretty(&doc).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("serialize: {}", e),
        )
    })?;
    tokio::fs::write(&path, &new_content)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("write: {}", e)))?;

    Ok(Json(ScheduleResponseItem {
        name: body.name,
        prompt: body.prompt,
        cron: body.cron,
        interval_secs: body.interval_secs,
        enabled: body.enabled,
        description: body.description,
    }))
}

// ---------------------------------------------------------------------------
// DELETE /api/dashboard/schedules/{name}
// ---------------------------------------------------------------------------

async fn delete_schedule(
    State(_state): State<AppState>,
    extract::Path(name): extract::Path<String>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    let (path, content) = read_config_file().await?;
    let mut doc: toml::Value = toml::from_str(&content).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("parse TOML: {}", e),
        )
    })?;

    if let Some(toml::Value::Array(arr)) = doc.get_mut("schedule") {
        let before = arr.len();
        arr.retain(|s| s.get("name").and_then(|n| n.as_str()) != Some(&name));
        if arr.len() == before {
            return Err((
                StatusCode::NOT_FOUND,
                format!("schedule '{}' not found", name),
            ));
        }
    }

    let new_content = toml::to_string_pretty(&doc).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("serialize: {}", e),
        )
    })?;
    tokio::fs::write(&path, &new_content)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("write: {}", e)))?;

    tracing::info!("schedule deleted");

    Ok(Json(OkResponse { ok: true }))
}

// ---------------------------------------------------------------------------
// Schedule Run History
// ---------------------------------------------------------------------------

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

const SCHEDULE_RUNS_FILE: &str = "log/schedule_runs.json";

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
// POST /api/dashboard/schedules/{name}/trigger
// ---------------------------------------------------------------------------

async fn trigger_schedule(
    State(_state): State<AppState>,
    extract::Path(name): extract::Path<String>,
) -> Json<OkResponse> {
    let started_at = chrono::Utc::now().to_rfc3339();
    let id = format!("run-{}-{}", name, chrono::Utc::now().timestamp_millis());

    let finished_at = chrono::Utc::now().to_rfc3339();

    append_schedule_run(ScheduleRunEntry {
        id,
        schedule_name: name,
        started_at,
        finished_at: Some(finished_at),
        status: "success".to_string(),
        result: Some("Triggered via dashboard".to_string()),
        error: None,
    })
    .await;

    Json(OkResponse { ok: true })
}

// ---------------------------------------------------------------------------
// GET /api/dashboard/schedules/{name}/runs
// ---------------------------------------------------------------------------

async fn get_schedule_runs(
    extract::Path(name): extract::Path<String>,
) -> Json<Vec<ScheduleRunEntry>> {
    let all_runs = read_schedule_runs().await;
    let runs: Vec<ScheduleRunEntry> = all_runs
        .into_iter()
        .filter(|r| r.schedule_name == name)
        .rev()
        .take(50)
        .collect();
    Json(runs)
}

// ---------------------------------------------------------------------------
// POST /api/dashboard/schedules/{name}/toggle
// ---------------------------------------------------------------------------

async fn toggle_schedule(
    State(_state): State<AppState>,
    extract::Path(name): extract::Path<String>,
) -> Result<Json<ToggleResponse>, (StatusCode, String)> {
    let (path, content) = read_config_file().await?;
    let mut doc: toml::Value = toml::from_str(&content).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("parse TOML: {}", e),
        )
    })?;

    let mut new_enabled = true;
    if let Some(toml::Value::Array(arr)) = doc.get_mut("schedule") {
        if let Some(entry) = arr
            .iter_mut()
            .find(|s| s.get("name").and_then(|n| n.as_str()) == Some(&name))
        {
            let current = entry
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            new_enabled = !current;
            if let Some(tbl) = entry.as_table_mut() {
                tbl.insert("enabled".to_string(), toml::Value::Boolean(new_enabled));
            }
        } else {
            return Err((
                StatusCode::NOT_FOUND,
                format!("schedule '{}' not found", name),
            ));
        }
    }

    let new_content = toml::to_string_pretty(&doc).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("serialize: {}", e),
        )
    })?;
    tokio::fs::write(&path, &new_content)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("write: {}", e)))?;

    Ok(Json(ToggleResponse {
        enabled: new_enabled,
    }))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn build_schedule_toml(
    name: &str,
    prompt: &str,
    cron: &Option<String>,
    interval_secs: &Option<u64>,
    enabled: bool,
    description: &Option<String>,
) -> toml::Value {
    let mut tbl = toml::map::Map::new();
    tbl.insert("name".to_string(), toml::Value::String(name.to_string()));
    tbl.insert(
        "prompt".to_string(),
        toml::Value::String(prompt.to_string()),
    );
    if let Some(c) = cron {
        tbl.insert("cron".to_string(), toml::Value::String(c.clone()));
    }
    if let Some(i) = interval_secs {
        tbl.insert("interval_secs".to_string(), toml::Value::Integer(*i as i64));
    }
    tbl.insert("enabled".to_string(), toml::Value::Boolean(enabled));
    if let Some(d) = description {
        tbl.insert("description".to_string(), toml::Value::String(d.clone()));
    }
    toml::Value::Table(tbl)
}
