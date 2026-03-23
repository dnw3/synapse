use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use axum::extract::{Path as AxumPath, Query, State};
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::{get, post, put};
use axum::Router;
use serde::{Deserialize, Serialize};
use synaptic::core::Tool;
use synaptic::mcp::MultiServerMcpClient;

use super::{config_file_path, OkResponse};
use crate::gateway::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/dashboard/mcp", get(get_mcp).post(create_mcp))
        .route("/dashboard/mcp/{name}", put(update_mcp).delete(delete_mcp))
        .route("/dashboard/mcp/{name}/test", post(test_mcp))
        .route("/dashboard/mcp/{name}/persist", post(persist_mcp))
        .route("/dashboard/requests", get(get_requests))
        .route("/dashboard/logs", get(get_logs))
        .route("/dashboard/logs/export", get(export_logs))
        .route("/dashboard/version", get(get_version))
}

// ---------------------------------------------------------------------------
// MCP response types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct McpServerInfoResponse {
    name: String,
    transport: String,
    command: Option<String>,
    args: Option<Vec<String>>,
    env: Option<HashMap<String, String>>,
    url: Option<String>,
    headers: Option<HashMap<String, String>>,
    status: String,
    tools: Vec<McpToolInfoResponse>,
    #[serde(rename = "lastChecked")]
    last_checked: Option<String>,
    error: Option<String>,
    transient: bool,
}

#[derive(Serialize)]
struct McpToolInfoResponse {
    name: String,
    #[serde(rename = "prefixedName")]
    prefixed_name: String,
    description: String,
    parameters: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// TOML read/write helpers
// ---------------------------------------------------------------------------

async fn read_config_toml() -> Result<(String, toml::Value), (StatusCode, String)> {
    let path = config_file_path();
    let content = tokio::fs::read_to_string(&path).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("read config: {}", e),
        )
    })?;
    let doc: toml::Value = toml::from_str(&content).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("parse config: {}", e),
        )
    })?;
    Ok((path, doc))
}

async fn write_config_toml(doc: &toml::Value) -> Result<(), (StatusCode, String)> {
    let path = config_file_path();
    let content = toml::to_string_pretty(doc).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("serialize config: {}", e),
        )
    })?;
    tokio::fs::write(&path, &content).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("write config: {}", e),
        )
    })?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Helper: build McpServerInfoResponse from config + tools
// ---------------------------------------------------------------------------

fn build_server_info(
    cfg: &synaptic::config::McpServerConfig,
    tools: &[Arc<dyn Tool>],
    transient: bool,
) -> McpServerInfoResponse {
    let status = if tools.is_empty() {
        "unknown".to_string()
    } else {
        "connected".to_string()
    };

    let tool_infos: Vec<McpToolInfoResponse> = tools
        .iter()
        .map(|t| McpToolInfoResponse {
            name: t
                .name()
                .strip_prefix(&format!("{}_", cfg.name))
                .unwrap_or(t.name())
                .to_string(),
            prefixed_name: t.name().to_string(),
            description: t.description().to_string(),
            parameters: t.parameters(),
        })
        .collect();

    McpServerInfoResponse {
        name: cfg.name.clone(),
        transport: cfg.transport.clone(),
        command: cfg.command.clone(),
        args: cfg.args.clone(),
        env: cfg.env.clone(),
        url: cfg.url.clone(),
        headers: cfg.headers.clone(),
        status,
        tools: tool_infos,
        last_checked: Some(chrono::Utc::now().to_rfc3339()),
        error: None,
        transient,
    }
}

// ---------------------------------------------------------------------------
// Helper: connect to an MCP server and load its tools
// ---------------------------------------------------------------------------

async fn connect_and_load_tools(
    cfg: &synaptic::config::McpServerConfig,
) -> Result<Vec<Arc<dyn Tool>>, String> {
    let conn = crate::agent::mcp::config_to_mcp_connection(cfg)
        .ok_or_else(|| format!("unsupported transport: {}", cfg.transport))?;

    let mut servers = HashMap::new();
    servers.insert(cfg.name.clone(), conn);
    let client = MultiServerMcpClient::new(servers);

    synaptic::mcp::load_mcp_tools(&client)
        .await
        .map_err(|e| format!("failed to connect: {}", e))
}

// ---------------------------------------------------------------------------
// Helper: build McpServerConfig from request fields
// ---------------------------------------------------------------------------

fn build_mcp_config(
    name: String,
    transport: String,
    command: Option<String>,
    args: Option<Vec<String>>,
    env: Option<HashMap<String, String>>,
    url: Option<String>,
    headers: Option<HashMap<String, String>>,
) -> synaptic::config::McpServerConfig {
    synaptic::config::McpServerConfig {
        name,
        transport,
        command,
        args,
        url,
        headers,
        env,
    }
}

// ---------------------------------------------------------------------------
// Helper: check if name already exists in persistent or transient servers
// ---------------------------------------------------------------------------

async fn name_exists(state: &AppState, name: &str) -> bool {
    // Check persistent
    if let Some(mcps) = &state.config.base.mcp {
        if mcps.iter().any(|m| m.name == name) {
            return true;
        }
    }
    // Check transient
    let transient = state.transient_mcp.read().await;
    transient.contains_key(name)
}

// ---------------------------------------------------------------------------
// Helper: append an MCP server entry to synapse.toml
// ---------------------------------------------------------------------------

async fn append_mcp_to_toml(
    cfg: &synaptic::config::McpServerConfig,
) -> Result<(), (StatusCode, String)> {
    let (_, mut doc) = read_config_toml().await?;

    let mut entry = toml::map::Map::new();
    entry.insert("name".into(), toml::Value::String(cfg.name.clone()));
    entry.insert(
        "transport".into(),
        toml::Value::String(cfg.transport.clone()),
    );
    if let Some(cmd) = &cfg.command {
        entry.insert("command".into(), toml::Value::String(cmd.clone()));
    }
    if let Some(args) = &cfg.args {
        entry.insert(
            "args".into(),
            toml::Value::Array(
                args.iter()
                    .map(|a| toml::Value::String(a.clone()))
                    .collect(),
            ),
        );
    }
    if let Some(url) = &cfg.url {
        entry.insert("url".into(), toml::Value::String(url.clone()));
    }
    if let Some(headers) = &cfg.headers {
        let mut tbl = toml::map::Map::new();
        for (k, v) in headers {
            tbl.insert(k.clone(), toml::Value::String(v.clone()));
        }
        entry.insert("headers".into(), toml::Value::Table(tbl));
    }
    if let Some(env) = &cfg.env {
        let mut tbl = toml::map::Map::new();
        for (k, v) in env {
            tbl.insert(k.clone(), toml::Value::String(v.clone()));
        }
        entry.insert("env".into(), toml::Value::Table(tbl));
    }

    // Get or create the [[mcp]] array
    let table = doc.as_table_mut().ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        "config not a table".into(),
    ))?;

    let mcp_array = table
        .entry("mcp")
        .or_insert_with(|| toml::Value::Array(Vec::new()));

    if let toml::Value::Array(arr) = mcp_array {
        arr.push(toml::Value::Table(entry));
    } else {
        return Err((
            StatusCode::INTERNAL_SERVER_ERROR,
            "mcp key is not an array".into(),
        ));
    }

    write_config_toml(&doc).await
}

// ---------------------------------------------------------------------------
// Helper: remove an MCP server entry from synapse.toml by name
// ---------------------------------------------------------------------------

async fn remove_mcp_from_toml(name: &str) -> Result<bool, (StatusCode, String)> {
    let (_, mut doc) = read_config_toml().await?;

    let table = doc.as_table_mut().ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        "config not a table".into(),
    ))?;

    if let Some(toml::Value::Array(arr)) = table.get_mut("mcp") {
        let before = arr.len();
        arr.retain(|entry| {
            entry
                .get("name")
                .and_then(|v| v.as_str())
                .map(|n| n != name)
                .unwrap_or(true)
        });
        if arr.len() < before {
            write_config_toml(&doc).await?;
            return Ok(true);
        }
    }
    Ok(false)
}

// ---------------------------------------------------------------------------
// GET /api/dashboard/mcp
// ---------------------------------------------------------------------------

async fn get_mcp(State(state): State<AppState>) -> Json<Vec<McpServerInfoResponse>> {
    let mut servers = Vec::new();

    // Persistent servers from config
    if let Some(mcps) = &state.config.base.mcp {
        let prefix_suffix = "_";
        for cfg in mcps {
            let prefix = format!("{}{}", cfg.name, prefix_suffix);
            let matching_tools: Vec<Arc<dyn Tool>> = state
                .mcp_tools
                .iter()
                .filter(|t| t.name().starts_with(&prefix))
                .cloned()
                .collect();
            servers.push(build_server_info(cfg, &matching_tools, false));
        }
    }

    // Transient servers
    {
        let transient = state.transient_mcp.read().await;
        for (_, (cfg, tools)) in transient.iter() {
            servers.push(build_server_info(cfg, tools, true));
        }
    }

    Json(servers)
}

// ---------------------------------------------------------------------------
// POST /api/dashboard/mcp (create)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct CreateMcpRequest {
    name: String,
    transport: String,
    command: Option<String>,
    args: Option<Vec<String>>,
    env: Option<HashMap<String, String>>,
    url: Option<String>,
    headers: Option<HashMap<String, String>>,
    #[serde(default)]
    transient: bool,
}

async fn create_mcp(
    State(state): State<AppState>,
    Json(body): Json<CreateMcpRequest>,
) -> Result<Json<McpServerInfoResponse>, (StatusCode, String)> {
    // Validate
    if body.name.trim().is_empty() {
        return Err((StatusCode::BAD_REQUEST, "name is required".into()));
    }
    if name_exists(&state, &body.name).await {
        return Err((
            StatusCode::CONFLICT,
            format!("MCP server '{}' already exists", body.name),
        ));
    }

    let cfg = build_mcp_config(
        body.name.clone(),
        body.transport.clone(),
        body.command.clone(),
        body.args.clone(),
        body.env.clone(),
        body.url.clone(),
        body.headers.clone(),
    );

    // Connect and load tools
    let tools = match connect_and_load_tools(&cfg).await {
        Ok(t) => t,
        Err(e) => {
            tracing::warn!(name = %cfg.name, error = %e, "MCP server connection failed");
            // Return info with error status
            let mut info = build_server_info(&cfg, &[], body.transient);
            info.status = "error".into();
            info.error = Some(e);
            return Ok(Json(info));
        }
    };

    tracing::info!(
        name = %cfg.name,
        tool_count = tools.len(),
        transient = body.transient,
        "MCP server connected"
    );

    if body.transient {
        let mut transient = state.transient_mcp.write().await;
        transient.insert(cfg.name.clone(), (cfg.clone(), tools.clone()));
    } else {
        // Append to synapse.toml
        append_mcp_to_toml(&cfg).await?;
    }

    Ok(Json(build_server_info(&cfg, &tools, body.transient)))
}

// ---------------------------------------------------------------------------
// PUT /api/dashboard/mcp/:name (update)
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct UpdateMcpRequest {
    transport: Option<String>,
    command: Option<String>,
    args: Option<Vec<String>>,
    env: Option<HashMap<String, String>>,
    url: Option<String>,
    headers: Option<HashMap<String, String>>,
}

async fn update_mcp(
    State(state): State<AppState>,
    AxumPath(name): AxumPath<String>,
    Json(body): Json<UpdateMcpRequest>,
) -> Result<Json<McpServerInfoResponse>, (StatusCode, String)> {
    // Check transient first
    {
        let transient = state.transient_mcp.read().await;
        if let Some((existing_cfg, _)) = transient.get(&name) {
            let cfg = build_mcp_config(
                name.clone(),
                body.transport
                    .unwrap_or_else(|| existing_cfg.transport.clone()),
                body.command.or_else(|| existing_cfg.command.clone()),
                body.args.or_else(|| existing_cfg.args.clone()),
                body.env.or_else(|| existing_cfg.env.clone()),
                body.url.or_else(|| existing_cfg.url.clone()),
                body.headers.or_else(|| existing_cfg.headers.clone()),
            );
            drop(transient);

            // Reconnect
            let tools = connect_and_load_tools(&cfg).await.map_err(|e| {
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    format!("reconnect failed: {}", e),
                )
            })?;

            let mut transient = state.transient_mcp.write().await;
            transient.insert(name.clone(), (cfg.clone(), tools.clone()));

            tracing::info!(name = %name, tool_count = tools.len(), "transient MCP server updated");
            return Ok(Json(build_server_info(&cfg, &tools, true)));
        }
    }

    // Check persistent
    let existing_cfg = state
        .config
        .base
        .mcp
        .as_ref()
        .and_then(|mcps| mcps.iter().find(|m| m.name == name))
        .cloned()
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("MCP server '{}' not found", name),
            )
        })?;

    let cfg = build_mcp_config(
        name.clone(),
        body.transport.unwrap_or(existing_cfg.transport),
        body.command.or(existing_cfg.command),
        body.args.or(existing_cfg.args),
        body.env.or(existing_cfg.env),
        body.url.or(existing_cfg.url),
        body.headers.or(existing_cfg.headers),
    );

    // Update TOML: remove old, add new
    remove_mcp_from_toml(&name).await?;
    append_mcp_to_toml(&cfg).await?;

    // Reconnect
    let tools = connect_and_load_tools(&cfg).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("reconnect failed: {}", e),
        )
    })?;

    tracing::info!(name = %name, tool_count = tools.len(), "persistent MCP server updated");
    Ok(Json(build_server_info(&cfg, &tools, false)))
}

// ---------------------------------------------------------------------------
// DELETE /api/dashboard/mcp/:name
// ---------------------------------------------------------------------------

async fn delete_mcp(
    State(state): State<AppState>,
    AxumPath(name): AxumPath<String>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    // Check transient first
    {
        let mut transient = state.transient_mcp.write().await;
        if transient.remove(&name).is_some() {
            tracing::info!(name = %name, "transient MCP server removed");
            return Ok(Json(OkResponse { ok: true }));
        }
    }

    // Remove from TOML
    let removed = remove_mcp_from_toml(&name).await?;
    if !removed {
        return Err((
            StatusCode::NOT_FOUND,
            format!("MCP server '{}' not found", name),
        ));
    }

    tracing::info!(name = %name, "persistent MCP server removed from config");
    Ok(Json(OkResponse { ok: true }))
}

// ---------------------------------------------------------------------------
// POST /api/dashboard/mcp/:name/test
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct McpTestResponse {
    success: bool,
    #[serde(rename = "toolCount")]
    tool_count: usize,
    #[serde(rename = "latencyMs")]
    latency_ms: u64,
    error: Option<String>,
}

async fn test_mcp(
    State(state): State<AppState>,
    AxumPath(name): AxumPath<String>,
) -> Json<McpTestResponse> {
    // Find the config (transient or persistent)
    let cfg = {
        let transient = state.transient_mcp.read().await;
        if let Some((cfg, _)) = transient.get(&name) {
            Some(cfg.clone())
        } else {
            state
                .config
                .base
                .mcp
                .as_ref()
                .and_then(|mcps| mcps.iter().find(|m| m.name == name))
                .cloned()
        }
    };

    let Some(cfg) = cfg else {
        return Json(McpTestResponse {
            success: false,
            tool_count: 0,
            latency_ms: 0,
            error: Some(format!("MCP server '{}' not found", name)),
        });
    };

    let start = std::time::Instant::now();
    match connect_and_load_tools(&cfg).await {
        Ok(tools) => {
            let latency = start.elapsed().as_millis() as u64;
            tracing::info!(
                name = %name,
                tool_count = tools.len(),
                latency_ms = latency,
                "MCP server test succeeded"
            );
            Json(McpTestResponse {
                success: true,
                tool_count: tools.len(),
                latency_ms: latency,
                error: None,
            })
        }
        Err(e) => {
            let latency = start.elapsed().as_millis() as u64;
            tracing::warn!(name = %name, error = %e, "MCP server test failed");
            Json(McpTestResponse {
                success: false,
                tool_count: 0,
                latency_ms: latency,
                error: Some(e),
            })
        }
    }
}

// ---------------------------------------------------------------------------
// POST /api/dashboard/mcp/:name/persist
// ---------------------------------------------------------------------------

async fn persist_mcp(
    State(state): State<AppState>,
    AxumPath(name): AxumPath<String>,
) -> Result<Json<McpServerInfoResponse>, (StatusCode, String)> {
    // Remove from transient
    let (cfg, tools) = {
        let mut transient = state.transient_mcp.write().await;
        transient.remove(&name).ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                format!("transient MCP server '{}' not found", name),
            )
        })?
    };

    // Append to TOML
    append_mcp_to_toml(&cfg).await?;

    tracing::info!(name = %name, "transient MCP server persisted to config");
    Ok(Json(build_server_info(&cfg, &tools, false)))
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
