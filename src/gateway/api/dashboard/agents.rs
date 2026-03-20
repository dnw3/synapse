use axum::extract::{self, State};
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::{delete, get, post, put};
use axum::Router;
use serde::{Deserialize, Serialize};

use super::{read_config_file, OkResponse};
use crate::gateway::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/dashboard/agents", get(get_agents))
        .route("/dashboard/agents", post(create_agent))
        .route("/dashboard/agents/{name}", put(update_agent))
        .route("/dashboard/agents/{name}", delete(delete_agent))
        .route("/dashboard/tools", get(get_tools_catalog))
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
