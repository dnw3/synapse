use std::collections::HashMap;
use std::path::{Path, PathBuf};

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::get;
use axum::Router;
use serde::{Deserialize, Serialize};

use crate::gateway::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/dashboard/mcp", get(get_mcp))
        .route("/dashboard/requests", get(get_requests))
        .route("/dashboard/logs", get(get_logs))
        .route("/dashboard/logs/export", get(export_logs))
        .route("/dashboard/version", get(get_version))
}

// ---------------------------------------------------------------------------
// GET /api/dashboard/mcp
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct McpServerResponse {
    name: String,
    transport: String,
    command: Option<String>,
    url: Option<String>,
}

async fn get_mcp(State(state): State<AppState>) -> Json<Vec<McpServerResponse>> {
    let servers = state
        .config
        .base
        .mcp
        .as_ref()
        .map(|mcps| {
            mcps.iter()
                .map(|m| McpServerResponse {
                    name: m.name.clone(),
                    transport: m.transport.clone(),
                    command: m.command.clone(),
                    url: m.url.clone(),
                })
                .collect()
        })
        .unwrap_or_default();

    Json(servers)
}

// ---------------------------------------------------------------------------
// GET /api/dashboard/requests
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct RequestMetricsResponse {
    endpoints: Vec<EndpointMetrics>,
    llm_durations: Vec<LlmDurationEntry>,
}

#[derive(Serialize)]
struct EndpointMetrics {
    method: String,
    path: String,
    total_requests: u64,
    status_counts: HashMap<u16, u64>,
    avg_duration_secs: Option<f64>,
}

#[derive(Serialize)]
struct LlmDurationEntry {
    model: String,
    count: u64,
    avg_duration_secs: f64,
}

async fn get_requests(State(state): State<AppState>) -> Json<RequestMetricsResponse> {
    let mut endpoint_map: HashMap<(String, String), (u64, HashMap<u16, u64>)> = HashMap::new();
    {
        let reqs = state.request_metrics.requests.read().await;
        for ((method, path, status), count) in reqs.iter() {
            let entry = endpoint_map
                .entry((method.clone(), path.clone()))
                .or_insert_with(|| (0, HashMap::new()));
            entry.0 += count;
            *entry.1.entry(*status).or_insert(0) += count;
        }
    }

    let durations = state.request_metrics.durations.read().await;
    let mut endpoints: Vec<EndpointMetrics> = endpoint_map
        .into_iter()
        .map(|((method, path), (total, status_counts))| {
            let avg = durations
                .get(&(method.clone(), path.clone()))
                .map(|(count, sum)| if *count > 0 { sum / *count as f64 } else { 0.0 });
            EndpointMetrics {
                method,
                path,
                total_requests: total,
                status_counts,
                avg_duration_secs: avg,
            }
        })
        .collect();
    endpoints.sort_by(|a, b| a.path.cmp(&b.path).then(a.method.cmp(&b.method)));

    let llm_durs = state.request_metrics.llm_durations.read().await;
    let mut llm_durations: Vec<LlmDurationEntry> = llm_durs
        .iter()
        .map(|(model, (count, sum))| LlmDurationEntry {
            model: model.clone(),
            count: *count,
            avg_duration_secs: if *count > 0 { sum / *count as f64 } else { 0.0 },
        })
        .collect();
    llm_durations.sort_by(|a, b| a.model.cmp(&b.model));

    Json(RequestMetricsResponse {
        endpoints,
        llm_durations,
    })
}

// ---------------------------------------------------------------------------
// GET /api/dashboard/logs
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct LogsQuery {
    lines: Option<usize>,
    level: Option<String>,
}

#[derive(Serialize)]
struct LogsResponse {
    lines: Vec<String>,
    file: Option<String>,
}

async fn get_logs(
    State(_state): State<AppState>,
    Query(query): Query<LogsQuery>,
) -> Result<Json<LogsResponse>, (StatusCode, String)> {
    let max_lines = query.lines.unwrap_or(100);
    let level_filter = query.level.as_deref();
    let log_dir = Path::new("log");

    if !log_dir.exists() {
        return Ok(Json(LogsResponse {
            lines: vec![],
            file: None,
        }));
    }

    let mut latest: Option<(PathBuf, std::time::SystemTime)> = None;
    if let Ok(mut entries) = tokio::fs::read_dir(log_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.is_file() {
                if let Ok(meta) = entry.metadata().await {
                    if let Ok(modified) = meta.modified() {
                        if latest.as_ref().map(|(_, t)| modified > *t).unwrap_or(true) {
                            latest = Some((path, modified));
                        }
                    }
                }
            }
        }
    }

    let Some((log_file, _)) = latest else {
        return Ok(Json(LogsResponse {
            lines: vec![],
            file: None,
        }));
    };

    let content = tokio::fs::read_to_string(&log_file).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("read log: {}", e),
        )
    })?;

    let all_lines: Vec<&str> = content.lines().collect();

    let filtered: Vec<&str> = if let Some(level) = level_filter {
        let level_upper = level.to_uppercase();
        all_lines
            .into_iter()
            .filter(|line| {
                let upper = line.to_uppercase();
                match level_upper.as_str() {
                    "ERROR" => upper.contains("ERROR"),
                    "WARN" => upper.contains("WARN"),
                    "INFO" => upper.contains("INFO"),
                    "DEBUG" => upper.contains("DEBUG") || upper.contains("TRACE"),
                    _ => true,
                }
            })
            .collect()
    } else {
        all_lines
    };

    let start = filtered.len().saturating_sub(max_lines);
    let lines: Vec<String> = filtered[start..].iter().map(|s| s.to_string()).collect();

    Ok(Json(LogsResponse {
        lines,
        file: Some(log_file.to_string_lossy().to_string()),
    }))
}

// ---------------------------------------------------------------------------
// GET /api/dashboard/logs/export
// ---------------------------------------------------------------------------

async fn export_logs(
    State(_state): State<AppState>,
) -> Result<(StatusCode, [(String, String); 2], Vec<u8>), (StatusCode, String)> {
    let log_dir = Path::new("log");

    if !log_dir.exists() {
        return Err((StatusCode::NOT_FOUND, "no log directory".to_string()));
    }

    let mut latest: Option<(PathBuf, std::time::SystemTime)> = None;
    if let Ok(mut entries) = tokio::fs::read_dir(log_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.is_file() {
                if let Ok(meta) = entry.metadata().await {
                    if let Ok(modified) = meta.modified() {
                        if latest.as_ref().map(|(_, t)| modified > *t).unwrap_or(true) {
                            latest = Some((path, modified));
                        }
                    }
                }
            }
        }
    }

    let Some((log_file, _)) = latest else {
        return Err((StatusCode::NOT_FOUND, "no log files found".to_string()));
    };

    let content = tokio::fs::read(&log_file)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("read: {}", e)))?;

    let filename = log_file
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "synapse.log".to_string());

    Ok((
        StatusCode::OK,
        [
            ("Content-Type".to_string(), "text/plain".to_string()),
            (
                "Content-Disposition".to_string(),
                format!("attachment; filename=\"{}\"", filename),
            ),
        ],
        content,
    ))
}

// ---------------------------------------------------------------------------
// GET /api/dashboard/version
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct VersionResponse {
    version: String,
    build_date: String,
}

async fn get_version() -> Json<VersionResponse> {
    Json(VersionResponse {
        version: env!("CARGO_PKG_VERSION").to_string(),
        build_date: chrono::Utc::now().format("%Y-%m-%d").to_string(),
    })
}
