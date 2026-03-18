use std::collections::HashMap;
use std::path::{Path, PathBuf};

use axum::extract::{self, Query, State};
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::{delete, get, post, put};
use axum::Router;
use serde::{Deserialize, Serialize};
use sha2::Digest;

use super::{read_config_file, sanitize_workspace_filename, OkResponse};
use crate::gateway::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        // Agents CRUD
        .route("/dashboard/agents", get(get_agents))
        .route("/dashboard/agents", post(create_agent))
        .route("/dashboard/agents/{name}", put(update_agent))
        .route("/dashboard/agents/{name}", delete(delete_agent))
        // Tools catalog
        .route("/dashboard/tools", get(get_tools_catalog))
        // MCP
        .route("/dashboard/mcp", get(get_mcp))
        // Requests/Metrics
        .route("/dashboard/requests", get(get_requests))
        // Logs
        .route("/dashboard/logs", get(get_logs))
        .route("/dashboard/logs/export", get(export_logs))
        // Debug
        .route("/dashboard/debug/invoke", post(debug_invoke))
        // Version
        .route("/dashboard/version", get(get_version))
        // Workspace files
        .route("/dashboard/workspace", get(get_workspace_files))
        .route("/dashboard/workspace/{filename}", get(get_workspace_file))
        .route("/dashboard/workspace/{filename}", put(put_workspace_file))
        .route(
            "/dashboard/workspace/{filename}",
            post(create_workspace_file),
        )
        .route(
            "/dashboard/workspace/{filename}",
            delete(delete_workspace_file),
        )
        .route(
            "/dashboard/workspace/{filename}/reset",
            post(reset_workspace_file),
        )
        // Identity
        .route("/dashboard/identity", get(get_identity))
        // Nodes & Device Pairing
        .route("/dashboard/nodes", get(get_nodes))
        .route("/dashboard/nodes/approve", post(approve_node))
        .route("/dashboard/nodes/reject", post(reject_node))
        .route("/dashboard/nodes/remove", post(remove_node))
        .route("/dashboard/nodes/rename", post(rename_node))
        .route("/dashboard/nodes/rotate", post(rotate_node_token))
        .route("/dashboard/nodes/revoke", post(revoke_node_token))
        .route("/dashboard/nodes/qr", post(generate_qr))
        .route("/dashboard/exec-approvals", get(get_exec_approvals))
}

// ---------------------------------------------------------------------------
// GET /api/dashboard/agents
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct AgentResponse {
    name: String,
    model: String,
    system_prompt: Option<String>,
    channels: Vec<String>,
    is_default: bool,
    workspace: Option<String>,
}

async fn get_agents(State(state): State<AppState>) -> Json<Vec<AgentResponse>> {
    let mut agents = Vec::new();

    agents.push(AgentResponse {
        name: "default".to_string(),
        model: state.config.base.model.model.clone(),
        system_prompt: state.config.base.agent.system_prompt.clone(),
        channels: vec![],
        workspace: Some(state.config.workspace_dir().to_string_lossy().to_string()),
        is_default: true,
    });

    if let Some(routes) = &state.config.agent_routes {
        for route in routes {
            agents.push(AgentResponse {
                name: route.name.clone(),
                model: route
                    .model
                    .clone()
                    .unwrap_or_else(|| state.config.base.model.model.clone()),
                system_prompt: route.system_prompt.clone(),
                channels: route.channels.clone(),
                is_default: false,
                workspace: Some(
                    state
                        .config
                        .workspace_dir_for_agent(Some(&route.name))
                        .to_string_lossy()
                        .to_string(),
                ),
            });
        }
    }

    Json(agents)
}

// ---------------------------------------------------------------------------
// POST /api/dashboard/agents
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct CreateAgentRequest {
    name: String,
    model: Option<String>,
    system_prompt: Option<String>,
    description: Option<String>,
    pattern: Option<String>,
    #[serde(default)]
    channels: Vec<String>,
    #[serde(default)]
    users: Vec<String>,
    priority: Option<u32>,
    workspace: Option<String>,
}

async fn create_agent(
    State(state): State<AppState>,
    Json(body): Json<CreateAgentRequest>,
) -> Result<Json<AgentResponse>, (StatusCode, String)> {
    if body.name == "default" {
        return Err((
            StatusCode::BAD_REQUEST,
            "cannot create agent named 'default'".to_string(),
        ));
    }

    let (path, content) = read_config_file().await?;
    let mut doc: toml::Value = toml::from_str(&content).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("parse TOML: {}", e),
        )
    })?;

    if let Some(toml::Value::Array(arr)) = doc.get("agent_routes") {
        if arr
            .iter()
            .any(|r| r.get("name").and_then(|n| n.as_str()) == Some(&body.name))
        {
            return Err((
                StatusCode::CONFLICT,
                format!("agent '{}' already exists", body.name),
            ));
        }
    }

    let new_entry = build_agent_route_toml(&body);

    let routes = doc
        .as_table_mut()
        .unwrap()
        .entry("agent_routes")
        .or_insert_with(|| toml::Value::Array(Vec::new()));
    if let toml::Value::Array(arr) = routes {
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

    Ok(Json(AgentResponse {
        name: body.name.clone(),
        model: body
            .model
            .unwrap_or_else(|| state.config.base.model.model.clone()),
        system_prompt: body.system_prompt,
        channels: body.channels,
        is_default: false,
        workspace: Some(
            state
                .config
                .workspace_dir_for_agent(Some(&body.name))
                .to_string_lossy()
                .to_string(),
        ),
    }))
}

// ---------------------------------------------------------------------------
// PUT /api/dashboard/agents/{name}
// ---------------------------------------------------------------------------

async fn update_agent(
    State(state): State<AppState>,
    extract::Path(name): extract::Path<String>,
    Json(body): Json<CreateAgentRequest>,
) -> Result<Json<AgentResponse>, (StatusCode, String)> {
    if name == "default" {
        return Err((
            StatusCode::BAD_REQUEST,
            "cannot modify the default agent via this endpoint".to_string(),
        ));
    }

    let (path, content) = read_config_file().await?;
    let mut doc: toml::Value = toml::from_str(&content).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("parse TOML: {}", e),
        )
    })?;

    if let Some(toml::Value::Array(arr)) = doc.get_mut("agent_routes") {
        if let Some(pos) = arr
            .iter()
            .position(|r| r.get("name").and_then(|n| n.as_str()) == Some(&name))
        {
            arr[pos] = build_agent_route_toml(&body);
        } else {
            return Err((StatusCode::NOT_FOUND, format!("agent '{}' not found", name)));
        }
    } else {
        return Err((
            StatusCode::NOT_FOUND,
            "no agent_routes configured".to_string(),
        ));
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

    tracing::info!(agent = %name, "agent updated");

    Ok(Json(AgentResponse {
        name: body.name.clone(),
        model: body
            .model
            .unwrap_or_else(|| state.config.base.model.model.clone()),
        system_prompt: body.system_prompt,
        channels: body.channels,
        is_default: false,
        workspace: Some(
            state
                .config
                .workspace_dir_for_agent(Some(&body.name))
                .to_string_lossy()
                .to_string(),
        ),
    }))
}

// ---------------------------------------------------------------------------
// DELETE /api/dashboard/agents/{name}
// ---------------------------------------------------------------------------

async fn delete_agent(
    State(_state): State<AppState>,
    extract::Path(name): extract::Path<String>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    if name == "default" {
        return Err((
            StatusCode::BAD_REQUEST,
            "cannot delete the default agent".to_string(),
        ));
    }

    let (path, content) = read_config_file().await?;
    let mut doc: toml::Value = toml::from_str(&content).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("parse TOML: {}", e),
        )
    })?;

    if let Some(toml::Value::Array(arr)) = doc.get_mut("agent_routes") {
        let before = arr.len();
        arr.retain(|r| r.get("name").and_then(|n| n.as_str()) != Some(&name));
        if arr.len() == before {
            return Err((StatusCode::NOT_FOUND, format!("agent '{}' not found", name)));
        }
    } else {
        return Err((StatusCode::NOT_FOUND, format!("agent '{}' not found", name)));
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

    Ok(Json(OkResponse { ok: true }))
}

// ---------------------------------------------------------------------------
// GET /api/dashboard/tools — Tool catalog
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct ToolCatalogEntry {
    name: String,
    description: String,
    source: String,
}

#[derive(Serialize)]
struct ToolCatalogGroup {
    id: String,
    label: String,
    tools: Vec<ToolCatalogEntry>,
}

async fn get_tools_catalog(State(state): State<AppState>) -> Json<Vec<ToolCatalogGroup>> {
    let mut groups = Vec::new();

    groups.push(ToolCatalogGroup {
        id: "filesystem".to_string(),
        label: "Filesystem".to_string(),
        tools: vec![
            ToolCatalogEntry {
                name: "ls".to_string(),
                description: "List directory contents".to_string(),
                source: "filesystem".to_string(),
            },
            ToolCatalogEntry {
                name: "read_file".to_string(),
                description: "Read file contents with optional line-based pagination".to_string(),
                source: "filesystem".to_string(),
            },
            ToolCatalogEntry {
                name: "write_file".to_string(),
                description: "Create or overwrite a file with the given content".to_string(),
                source: "filesystem".to_string(),
            },
            ToolCatalogEntry {
                name: "edit_file".to_string(),
                description: "Find and replace text in a file".to_string(),
                source: "filesystem".to_string(),
            },
            ToolCatalogEntry {
                name: "glob".to_string(),
                description: "Find files matching a glob pattern".to_string(),
                source: "filesystem".to_string(),
            },
            ToolCatalogEntry {
                name: "grep".to_string(),
                description: "Search file contents by regex pattern".to_string(),
                source: "filesystem".to_string(),
            },
            ToolCatalogEntry {
                name: "execute".to_string(),
                description: "Execute a shell command".to_string(),
                source: "filesystem".to_string(),
            },
        ],
    });

    #[allow(unused_mut)]
    let mut core_tools = vec![
        ToolCatalogEntry {
            name: "apply_patch".to_string(),
            description: "Apply a unified diff patch to a file".to_string(),
            source: "core".to_string(),
        },
        ToolCatalogEntry {
            name: "read_pdf".to_string(),
            description: "Read and extract text from a PDF file".to_string(),
            source: "core".to_string(),
        },
        ToolCatalogEntry {
            name: "firecrawl".to_string(),
            description: "Crawl and extract content from web pages".to_string(),
            source: "core".to_string(),
        },
        ToolCatalogEntry {
            name: "analyze_image".to_string(),
            description: "Analyze image content using a vision model".to_string(),
            source: "core".to_string(),
        },
    ];

    #[cfg(feature = "voice")]
    {
        if std::env::var("OPENAI_API_KEY").is_ok() {
            core_tools.push(ToolCatalogEntry {
                name: "transcribe_audio".to_string(),
                description: "Transcribe audio files to text using speech-to-text".to_string(),
                source: "core".to_string(),
            });
        }
    }

    #[cfg(feature = "browser")]
    {
        core_tools.push(ToolCatalogEntry {
            name: "browser".to_string(),
            description: "Browser automation tools for web interaction".to_string(),
            source: "core".to_string(),
        });
    }

    groups.push(ToolCatalogGroup {
        id: "core".to_string(),
        label: "Core".to_string(),
        tools: core_tools,
    });

    groups.push(ToolCatalogGroup {
        id: "agent".to_string(),
        label: "Agent".to_string(),
        tools: vec![
            ToolCatalogEntry {
                name: "Skill".to_string(),
                description: "Execute a skill by name with arguments".to_string(),
                source: "agent".to_string(),
            },
            ToolCatalogEntry {
                name: "task".to_string(),
                description: "Spawn a sub-agent to handle a delegated task".to_string(),
                source: "agent".to_string(),
            },
            ToolCatalogEntry {
                name: "TaskOutput".to_string(),
                description: "Retrieve output from a background task".to_string(),
                source: "agent".to_string(),
            },
            ToolCatalogEntry {
                name: "llm_task".to_string(),
                description: "Lightweight LLM delegation for simple queries".to_string(),
                source: "agent".to_string(),
            },
        ],
    });

    if state.config.memory.ltm_enabled {
        groups.push(ToolCatalogGroup {
            id: "memory".to_string(),
            label: "Memory".to_string(),
            tools: vec![
                ToolCatalogEntry {
                    name: "memory_search".to_string(),
                    description: "Search long-term memory by semantic query".to_string(),
                    source: "memory".to_string(),
                },
                ToolCatalogEntry {
                    name: "memory_get".to_string(),
                    description: "Retrieve a specific memory entry by key".to_string(),
                    source: "memory".to_string(),
                },
            ],
        });
    }

    groups.push(ToolCatalogGroup {
        id: "session".to_string(),
        label: "Session".to_string(),
        tools: vec![
            ToolCatalogEntry {
                name: "sessions_list".to_string(),
                description: "List active sessions".to_string(),
                source: "session".to_string(),
            },
            ToolCatalogEntry {
                name: "sessions_history".to_string(),
                description: "Get message history for a session".to_string(),
                source: "session".to_string(),
            },
            ToolCatalogEntry {
                name: "sessions_send".to_string(),
                description: "Send a message to another session".to_string(),
                source: "session".to_string(),
            },
            ToolCatalogEntry {
                name: "sessions_spawn".to_string(),
                description: "Spawn a new session with a prompt".to_string(),
                source: "session".to_string(),
            },
        ],
    });

    if let Some(ref mcp_servers) = state.config.base.mcp {
        let mut mcp_tools = Vec::new();
        for server in mcp_servers {
            let desc = server
                .command
                .as_deref()
                .or(server.url.as_deref())
                .unwrap_or("MCP server")
                .to_string();
            mcp_tools.push(ToolCatalogEntry {
                name: server.name.clone(),
                description: format!("MCP: {}", desc),
                source: "mcp".to_string(),
            });
        }
        if !mcp_tools.is_empty() {
            groups.push(ToolCatalogGroup {
                id: "mcp".to_string(),
                label: "MCP Servers".to_string(),
                tools: mcp_tools,
            });
        }
    }

    Json(groups)
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
// POST /api/dashboard/debug/invoke
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct DebugInvokeRequest {
    method: String,
    #[allow(dead_code)]
    params: serde_json::Value,
}

#[derive(Serialize)]
struct DebugInvokeResponse {
    ok: bool,
    result: Option<serde_json::Value>,
    error: Option<String>,
}

async fn debug_invoke(
    State(state): State<AppState>,
    Json(body): Json<DebugInvokeRequest>,
) -> Json<DebugInvokeResponse> {
    match body.method.as_str() {
        "health" => {
            let uptime = state.started_at.elapsed().as_secs();
            let active = state.cancel_tokens.read().await.len();
            Json(DebugInvokeResponse {
                ok: true,
                result: Some(serde_json::json!({
                    "status": "ok",
                    "uptime_secs": uptime,
                    "active_connections": active,
                })),
                error: None,
            })
        }
        "cost_snapshot" => {
            let snapshot = state.cost_tracker.snapshot().await;
            Json(DebugInvokeResponse {
                ok: true,
                result: Some(serde_json::json!({
                    "total_input_tokens": snapshot.total_input_tokens,
                    "total_output_tokens": snapshot.total_output_tokens,
                    "total_cost_usd": snapshot.estimated_cost_usd,
                    "total_requests": snapshot.total_requests,
                })),
                error: None,
            })
        }
        "stats" => {
            let snapshot = state.cost_tracker.snapshot().await;
            let sessions = state
                .sessions
                .list_sessions()
                .await
                .map(|s| s.len())
                .unwrap_or(0);
            let active = state.cancel_tokens.read().await.len();
            Json(DebugInvokeResponse {
                ok: true,
                result: Some(serde_json::json!({
                    "session_count": sessions,
                    "total_input_tokens": snapshot.total_input_tokens,
                    "total_output_tokens": snapshot.total_output_tokens,
                    "total_cost_usd": snapshot.estimated_cost_usd,
                    "total_requests": snapshot.total_requests,
                    "active_ws_sessions": active,
                    "uptime_secs": state.started_at.elapsed().as_secs(),
                })),
                error: None,
            })
        }
        "version" => Json(DebugInvokeResponse {
            ok: true,
            result: Some(serde_json::json!({
                "version": env!("CARGO_PKG_VERSION"),
                "name": env!("CARGO_PKG_NAME"),
            })),
            error: None,
        }),
        "providers" => {
            let mut providers = Vec::new();
            if let Some(catalog) = &state.config.provider_catalog {
                for p in catalog {
                    providers.push(serde_json::json!({
                        "name": p.name,
                        "base_url": p.base_url,
                    }));
                }
            }
            Json(DebugInvokeResponse {
                ok: true,
                result: Some(serde_json::json!(providers)),
                error: None,
            })
        }
        "models.list" => {
            let mut models = Vec::new();
            if let Some(catalog) = &state.config.model_catalog {
                for m in catalog {
                    models.push(serde_json::json!({
                        "name": m.name,
                        "provider": m.provider,
                    }));
                }
            }
            Json(DebugInvokeResponse {
                ok: true,
                result: Some(serde_json::json!(models)),
                error: None,
            })
        }
        "sessions" => {
            let sessions = state.sessions.list_sessions().await.unwrap_or_default();
            let list: Vec<_> = sessions
                .iter()
                .map(|s| {
                    serde_json::json!({
                        "id": s.session_id,
                    })
                })
                .collect();
            Json(DebugInvokeResponse {
                ok: true,
                result: Some(serde_json::json!(list)),
                error: None,
            })
        }
        "schedules" => {
            let schedules: Vec<_> = state
                .config
                .schedules
                .as_ref()
                .map(|entries| {
                    entries
                        .iter()
                        .map(|s| {
                            serde_json::json!({
                                "name": s.name,
                                "prompt": s.prompt,
                                "cron": s.cron,
                                "interval_secs": s.interval_secs,
                                "enabled": s.enabled,
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            Json(DebugInvokeResponse {
                ok: true,
                result: Some(serde_json::json!(schedules)),
                error: None,
            })
        }
        _ => {
            let rpc_ctx = std::sync::Arc::new(crate::gateway::rpc::router::RpcContext {
                state: state.clone(),
                conn_id: "dashboard-rest".to_string(),
                client: crate::gateway::rpc::types::ClientInfo::default(),
                role: crate::gateway::rpc::scopes::Role::Operator,
                scopes: std::collections::HashSet::from([
                    "operator.read".to_string(),
                    "operator.write".to_string(),
                    "operator.pairing".to_string(),
                    "operator.approvals".to_string(),
                ]),
                broadcaster: state.broadcaster.clone(),
            });
            let frame = state
                .rpc_router
                .dispatch(
                    rpc_ctx,
                    "dashboard-rest-0".to_string(),
                    &body.method,
                    body.params.clone(),
                )
                .await;
            match frame {
                crate::gateway::rpc::types::ServerFrame::Response {
                    ok, payload, error, ..
                } => Json(DebugInvokeResponse {
                    ok,
                    result: payload,
                    error: error.map(|e| e.message),
                }),
                _ => Json(DebugInvokeResponse {
                    ok: false,
                    result: None,
                    error: Some("unexpected RPC response".to_string()),
                }),
            }
        }
    }
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

// ---------------------------------------------------------------------------
// Workspace files CRUD
// ---------------------------------------------------------------------------

fn workspace_dir(config: &crate::config::SynapseConfig, agent: Option<&str>) -> PathBuf {
    config.workspace_dir_for_agent(agent)
}

#[derive(Deserialize)]
struct WorkspaceQuery {
    agent: Option<String>,
}

#[derive(Serialize)]
struct WorkspaceFileEntry {
    filename: String,
    description: String,
    category: String,
    icon: String,
    exists: bool,
    size_bytes: Option<u64>,
    modified: Option<String>,
    preview: Option<String>,
    is_template: bool,
}

async fn get_workspace_files(
    State(state): State<AppState>,
    Query(query): Query<WorkspaceQuery>,
) -> Json<Vec<WorkspaceFileEntry>> {
    use crate::agent::templates::WORKSPACE_TEMPLATES;

    let cwd = workspace_dir(&state.config, query.agent.as_deref());
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

        entries.push(WorkspaceFileEntry {
            filename: tmpl.filename.to_string(),
            description: tmpl.description.to_string(),
            category: tmpl.category.to_string(),
            icon: tmpl.icon.to_string(),
            exists,
            size_bytes,
            modified,
            preview,
            is_template: true,
        });
    }

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
            entries.push(WorkspaceFileEntry {
                filename: name,
                description: "Custom workspace file".to_string(),
                category: "custom".to_string(),
                icon: "file-text".to_string(),
                exists: true,
                size_bytes: size,
                modified: mod_time,
                preview,
                is_template: false,
            });
        }
    }

    Json(entries)
}

#[derive(Serialize)]
struct WorkspaceFileContent {
    filename: String,
    content: String,
    is_template: bool,
}

async fn get_workspace_file(
    State(state): State<AppState>,
    extract::Path(filename): extract::Path<String>,
    Query(query): Query<WorkspaceQuery>,
) -> Result<Json<WorkspaceFileContent>, (StatusCode, String)> {
    sanitize_workspace_filename(&filename)?;
    let path = workspace_dir(&state.config, query.agent.as_deref()).join(&filename);
    let content = tokio::fs::read_to_string(&path).await.map_err(|_| {
        (
            StatusCode::NOT_FOUND,
            format!("file '{}' not found", filename),
        )
    })?;
    let is_template = crate::agent::templates::find_template(&filename).is_some();
    Ok(Json(WorkspaceFileContent {
        filename,
        content,
        is_template,
    }))
}

#[derive(Deserialize)]
struct WorkspaceFileBody {
    content: String,
}

async fn put_workspace_file(
    State(state): State<AppState>,
    extract::Path(filename): extract::Path<String>,
    Query(query): Query<WorkspaceQuery>,
    Json(body): Json<WorkspaceFileBody>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    sanitize_workspace_filename(&filename)?;
    let path = workspace_dir(&state.config, query.agent.as_deref()).join(&filename);
    if !path.exists() {
        return Err((
            StatusCode::NOT_FOUND,
            format!("file '{}' not found \u{2014} use POST to create", filename),
        ));
    }
    tokio::fs::write(&path, &body.content)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("write: {}", e)))?;

    tracing::info!(file = %filename, "workspace file saved");

    Ok(Json(OkResponse { ok: true }))
}

async fn create_workspace_file(
    State(state): State<AppState>,
    extract::Path(filename): extract::Path<String>,
    Query(query): Query<WorkspaceQuery>,
    Json(body): Json<WorkspaceFileBody>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    sanitize_workspace_filename(&filename)?;
    let path = workspace_dir(&state.config, query.agent.as_deref()).join(&filename);
    if path.exists() {
        return Err((
            StatusCode::CONFLICT,
            format!(
                "file '{}' already exists \u{2014} use PUT to update",
                filename
            ),
        ));
    }
    tokio::fs::write(&path, &body.content)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("write: {}", e)))?;
    Ok(Json(OkResponse { ok: true }))
}

async fn delete_workspace_file(
    State(state): State<AppState>,
    extract::Path(filename): extract::Path<String>,
    Query(query): Query<WorkspaceQuery>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    sanitize_workspace_filename(&filename)?;
    let path = workspace_dir(&state.config, query.agent.as_deref()).join(&filename);
    if !path.exists() {
        return Err((
            StatusCode::NOT_FOUND,
            format!("file '{}' not found", filename),
        ));
    }
    tokio::fs::remove_file(&path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("delete: {}", e)))?;
    Ok(Json(OkResponse { ok: true }))
}

async fn reset_workspace_file(
    State(state): State<AppState>,
    extract::Path(filename): extract::Path<String>,
    Query(query): Query<WorkspaceQuery>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    sanitize_workspace_filename(&filename)?;
    let default = crate::agent::workspace::default_content_for(&filename).ok_or_else(|| {
        (
            StatusCode::NOT_FOUND,
            format!("no default template for '{}'", filename),
        )
    })?;
    let path = workspace_dir(&state.config, query.agent.as_deref()).join(&filename);
    tokio::fs::write(&path, default)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("write: {}", e)))?;
    Ok(Json(OkResponse { ok: true }))
}

async fn get_identity(
    State(state): State<AppState>,
    Query(query): Query<WorkspaceQuery>,
) -> Json<crate::agent::workspace::IdentityInfo> {
    let path = workspace_dir(&state.config, query.agent.as_deref()).join("IDENTITY.md");
    let info = match tokio::fs::read_to_string(&path).await {
        Ok(content) => crate::agent::workspace::parse_identity(&content),
        Err(_) => crate::agent::workspace::IdentityInfo::default(),
    };
    Json(info)
}

// ---------------------------------------------------------------------------
// Nodes & Device Pairing
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct NodesResponse {
    nodes: Vec<serde_json::Value>,
    pending: Vec<serde_json::Value>,
}

async fn get_nodes(State(state): State<AppState>) -> Json<NodesResponse> {
    let paired = {
        let pairing = state.pairing_store.read().await;
        pairing.list_paired()
    };

    let registry = state.node_registry.read().await;
    let nodes: Vec<serde_json::Value> = paired
        .iter()
        .map(|n| {
            let session = registry.get(&n.node_id);
            let online = session.is_some();
            let token_status = match &n.token_hash {
                Some(h) if h.is_empty() => "revoked",
                Some(_) => "active",
                None => "none",
            };
            serde_json::json!({
                "id": n.node_id,
                "name": n.name,
                "platform": n.platform,
                "status": if online { "online" } else { "offline" },
                "paired_at": n.paired_at.to_string(),
                "device_id": n.device_id,
                "token_status": token_status,
                "connected_at": session.map(|s| s.connected_at),
                "capabilities": session.map(|s| &s.capabilities),
            })
        })
        .collect();
    drop(registry);

    let mut pairing_w = state.pairing_store.write().await;
    let pending_list = pairing_w.list_pending();
    let pending: Vec<serde_json::Value> = pending_list
        .iter()
        .map(|r| {
            serde_json::json!({
                "id": r.request_id,
                "node_name": r.node_name,
                "platform": r.platform,
                "ip": r.ip,
                "requested_at": r.created_at.to_string(),
            })
        })
        .collect();

    Json(NodesResponse { nodes, pending })
}

#[derive(Deserialize)]
struct NodeActionRequest {
    request_id: Option<String>,
    node_id: Option<String>,
}

async fn approve_node(
    State(state): State<AppState>,
    Json(body): Json<NodeActionRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let request_id = body
        .request_id
        .as_deref()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing request_id".to_string()))?;
    let paired = state
        .pairing_store
        .write()
        .await
        .approve(request_id)
        .ok_or_else(|| {
            (
                StatusCode::NOT_FOUND,
                "pending request not found".to_string(),
            )
        })?;
    Ok(Json(serde_json::to_value(&paired).unwrap_or_default()))
}

async fn reject_node(
    State(state): State<AppState>,
    Json(body): Json<NodeActionRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let request_id = body
        .request_id
        .as_deref()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing request_id".to_string()))?;
    let removed = state.pairing_store.write().await.reject(request_id);
    if removed {
        Ok(Json(serde_json::json!({"ok": true})))
    } else {
        Err((
            StatusCode::NOT_FOUND,
            "pending request not found".to_string(),
        ))
    }
}

async fn remove_node(
    State(state): State<AppState>,
    Json(body): Json<NodeActionRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let node_id = body
        .node_id
        .as_deref()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing node_id".to_string()))?;
    let removed = state.pairing_store.write().await.remove_paired(node_id);
    if removed {
        state.node_registry.write().await.unregister(node_id);
        Ok(Json(serde_json::json!({"ok": true})))
    } else {
        Err((StatusCode::NOT_FOUND, "paired device not found".to_string()))
    }
}

#[derive(Deserialize)]
struct RenameRequest {
    node_id: String,
    name: String,
}

async fn rename_node(
    State(state): State<AppState>,
    Json(body): Json<RenameRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let renamed = state
        .pairing_store
        .write()
        .await
        .rename(&body.node_id, &body.name);
    if renamed {
        state
            .node_registry
            .write()
            .await
            .rename(&body.node_id, &body.name);
        Ok(Json(serde_json::json!({"ok": true})))
    } else {
        Err((StatusCode::NOT_FOUND, "device not found".to_string()))
    }
}

async fn rotate_node_token(
    State(state): State<AppState>,
    Json(body): Json<NodeActionRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    use crate::gateway::nodes::bootstrap;

    let node_id = body
        .node_id
        .as_deref()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing node_id".to_string()))?;

    let new_token = bootstrap::generate_pairing_token();
    let token_hash = format!("{:x}", sha2::Sha256::digest(new_token.as_bytes()));

    let updated = state
        .pairing_store
        .write()
        .await
        .update_token_hash(node_id, &token_hash);
    if updated {
        Ok(Json(serde_json::json!({
            "ok": true,
            "token": new_token,
        })))
    } else {
        Err((StatusCode::NOT_FOUND, "device not found".to_string()))
    }
}

async fn revoke_node_token(
    State(state): State<AppState>,
    Json(body): Json<NodeActionRequest>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let node_id = body
        .node_id
        .as_deref()
        .ok_or_else(|| (StatusCode::BAD_REQUEST, "missing node_id".to_string()))?;

    let updated = state
        .pairing_store
        .write()
        .await
        .update_token_hash(node_id, "");
    if updated {
        Ok(Json(serde_json::json!({"ok": true})))
    } else {
        Err((StatusCode::NOT_FOUND, "device not found".to_string()))
    }
}

#[derive(Deserialize)]
struct QrRequest {
    url: Option<String>,
}

async fn generate_qr(
    State(state): State<AppState>,
    Json(body): Json<QrRequest>,
) -> Json<serde_json::Value> {
    use crate::gateway::nodes::bootstrap;

    let token = state.bootstrap_store.write().await.issue();

    let gateway_url = body.url.unwrap_or_else(|| {
        let port = state
            .config
            .serve
            .as_ref()
            .and_then(|s| s.port)
            .unwrap_or(3000);
        format!("ws://localhost:{}", port)
    });

    let setup_code = bootstrap::encode_setup_code(&gateway_url, &token);
    let qr_svg = bootstrap::generate_qr_svg(&setup_code).unwrap_or_default();

    Json(serde_json::json!({
        "ok": true,
        "setup_code": setup_code,
        "qr_svg": qr_svg,
        "gateway_url": gateway_url,
        "bootstrap_token": token,
        "ttl_ms": 10 * 60 * 1000,
    }))
}

async fn get_exec_approvals(State(state): State<AppState>) -> Json<serde_json::Value> {
    let config = state.exec_approvals_config.read().await;
    Json(serde_json::json!({
        "security_mode": config.mode,
        "ask_policy": config.ask,
        "allowlist": config.allowlist,
    }))
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn build_agent_route_toml(body: &CreateAgentRequest) -> toml::Value {
    let mut tbl = toml::map::Map::new();
    tbl.insert("name".to_string(), toml::Value::String(body.name.clone()));
    if let Some(ref model) = body.model {
        tbl.insert("model".to_string(), toml::Value::String(model.clone()));
    }
    if let Some(ref sp) = body.system_prompt {
        tbl.insert("system_prompt".to_string(), toml::Value::String(sp.clone()));
    }
    if let Some(ref desc) = body.description {
        tbl.insert("description".to_string(), toml::Value::String(desc.clone()));
    }
    if let Some(ref pattern) = body.pattern {
        tbl.insert("pattern".to_string(), toml::Value::String(pattern.clone()));
    }
    if !body.channels.is_empty() {
        tbl.insert(
            "channels".to_string(),
            toml::Value::Array(
                body.channels
                    .iter()
                    .map(|c| toml::Value::String(c.clone()))
                    .collect(),
            ),
        );
    }
    if !body.users.is_empty() {
        tbl.insert(
            "users".to_string(),
            toml::Value::Array(
                body.users
                    .iter()
                    .map(|u| toml::Value::String(u.clone()))
                    .collect(),
            ),
        );
    }
    if let Some(priority) = body.priority {
        tbl.insert(
            "priority".to_string(),
            toml::Value::Integer(priority as i64),
        );
    }
    if let Some(ref workspace) = body.workspace {
        tbl.insert(
            "workspace".to_string(),
            toml::Value::String(workspace.clone()),
        );
    }
    toml::Value::Table(tbl)
}
