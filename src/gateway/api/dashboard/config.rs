use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::{get, post, put};
use axum::Router;
use serde::{Deserialize, Serialize};

use super::{config_file_path, read_config_file, OkResponse};
use crate::gateway::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/dashboard/config", get(get_config))
        .route("/dashboard/config", put(put_config))
        .route("/dashboard/config/schema", get(get_config_schema))
        .route("/dashboard/config/validate", post(validate_config))
        .route("/dashboard/config/reload", post(reload_config))
}

// ---------------------------------------------------------------------------
// GET /api/dashboard/config
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct ConfigResponse {
    content: String,
    path: String,
}

async fn get_config(
    State(_state): State<AppState>,
) -> Result<Json<ConfigResponse>, (StatusCode, String)> {
    let (path, content) = read_config_file().await?;
    Ok(Json(ConfigResponse { content, path }))
}

// ---------------------------------------------------------------------------
// PUT /api/dashboard/config
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ConfigUpdateRequest {
    content: String,
}

#[derive(Serialize)]
struct ConfigUpdateResponse {
    success: bool,
    path: String,
}

async fn put_config(
    State(_state): State<AppState>,
    Json(body): Json<ConfigUpdateRequest>,
) -> Result<Json<ConfigUpdateResponse>, (StatusCode, String)> {
    toml::from_str::<toml::Value>(&body.content)
        .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid TOML: {}", e)))?;

    let path = config_file_path();
    tokio::fs::write(&path, &body.content).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("write failed: {}", e),
        )
    })?;

    Ok(Json(ConfigUpdateResponse {
        success: true,
        path,
    }))
}

// ---------------------------------------------------------------------------
// POST /api/dashboard/config/validate
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ValidateConfigRequest {
    content: String,
}

#[derive(Serialize)]
struct ValidateConfigResponse {
    valid: bool,
    errors: Vec<String>,
}

async fn validate_config(
    State(_state): State<AppState>,
    Json(body): Json<ValidateConfigRequest>,
) -> Json<ValidateConfigResponse> {
    let toml_val = match toml::from_str::<toml::Value>(&body.content) {
        Ok(v) => v,
        Err(e) => {
            return Json(ValidateConfigResponse {
                valid: false,
                errors: vec![format!("TOML syntax: {}", e)],
            });
        }
    };

    let mut errors = Vec::new();
    match toml::from_str::<crate::config::SynapseConfig>(&body.content) {
        Ok(_) => {}
        Err(e) => {
            errors.push(format!("Config structure: {}", e));
        }
    }

    if let Some(table) = toml_val.as_table() {
        fn check_sensitive(
            table: &toml::map::Map<String, toml::Value>,
            path: &str,
            warnings: &mut Vec<String>,
        ) {
            let sensitive_keys = [
                "api_key",
                "token",
                "secret",
                "password",
                "app_secret",
                "signing_secret",
                "bot_token",
            ];
            for (k, v) in table {
                let full_path = if path.is_empty() {
                    k.clone()
                } else {
                    format!("{}.{}", path, k)
                };
                if sensitive_keys.iter().any(|s| k.contains(s)) {
                    if let toml::Value::String(val) = v {
                        if !val.is_empty() && !val.starts_with("${") && !val.ends_with("_env") {
                            warnings.push(format!("Sensitive value in clear text: {}", full_path));
                        }
                    }
                }
                if let toml::Value::Table(sub) = v {
                    check_sensitive(sub, &full_path, warnings);
                }
            }
        }
        check_sensitive(table, "", &mut errors);
    }

    Json(ValidateConfigResponse {
        valid: errors.is_empty(),
        errors,
    })
}

// ---------------------------------------------------------------------------
// POST /api/dashboard/config/reload
// ---------------------------------------------------------------------------

async fn reload_config(State(_state): State<AppState>) -> Json<OkResponse> {
    Json(OkResponse { ok: true })
}

// ---------------------------------------------------------------------------
// GET /api/dashboard/config/schema
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone)]
struct ConfigFieldSchema {
    key: String,
    label: String,
    #[serde(rename = "type")]
    field_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    placeholder: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    options: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    default_value: Option<String>,
    sensitive: bool,
}

#[derive(Serialize, Clone)]
struct ConfigSectionSchema {
    key: String,
    label: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    description: Option<String>,
    order: u32,
    icon: String,
    fields: Vec<ConfigFieldSchema>,
}

#[derive(Serialize)]
struct ConfigSchemaResponse {
    sections: Vec<ConfigSectionSchema>,
    sensitive_patterns: Vec<String>,
}

fn field(key: &str, label: &str, ft: &str) -> ConfigFieldSchema {
    ConfigFieldSchema {
        key: key.to_string(),
        label: label.to_string(),
        field_type: ft.to_string(),
        description: None,
        placeholder: None,
        options: None,
        default_value: None,
        sensitive: false,
    }
}

fn build_config_schema() -> Vec<ConfigSectionSchema> {
    vec![
        ConfigSectionSchema {
            key: "model".into(),
            label: "Model".into(),
            description: Some("Primary LLM model configuration".into()),
            order: 10,
            icon: "brain".into(),
            fields: vec![
                {
                    let mut f = field("provider", "Provider", "enum");
                    f.description = Some("LLM provider".into());
                    f.options = Some(
                        vec![
                            "openai",
                            "anthropic",
                            "gemini",
                            "ollama",
                            "bedrock",
                            "deepseek",
                            "groq",
                            "mistral",
                            "together",
                            "fireworks",
                            "xai",
                            "perplexity",
                            "cohere",
                            "ark",
                        ]
                        .into_iter()
                        .map(String::from)
                        .collect(),
                    );
                    f.default_value = Some("openai".into());
                    f
                },
                {
                    let mut f = field("model", "Model Name", "string");
                    f.description = Some("Model identifier".into());
                    f.placeholder = Some("gpt-4o".into());
                    f.default_value = Some("gpt-4o".into());
                    f
                },
                {
                    let mut f = field("api_key_env", "API Key Env Var", "string");
                    f.description = Some("Environment variable containing the API key".into());
                    f.placeholder = Some("OPENAI_API_KEY".into());
                    f
                },
                {
                    let mut f = field("api_key", "API Key", "secret");
                    f.description = Some("Direct API key (prefer api_key_env)".into());
                    f.sensitive = true;
                    f
                },
                {
                    let mut f = field("base_url", "Base URL", "string");
                    f.description = Some("Custom API endpoint URL".into());
                    f.placeholder = Some("https://api.openai.com/v1".into());
                    f
                },
                {
                    let mut f = field("temperature", "Temperature", "number");
                    f.description = Some("Sampling temperature (0.0-2.0)".into());
                    f.default_value = Some("0.7".into());
                    f
                },
                {
                    let mut f = field("max_tokens", "Max Tokens", "number");
                    f.description = Some("Maximum tokens in response".into());
                    f.default_value = Some("4096".into());
                    f
                },
            ],
        },
        ConfigSectionSchema {
            key: "agent".into(),
            label: "Agent".into(),
            description: Some("Agent behavior and tool configuration".into()),
            order: 20,
            icon: "bot".into(),
            fields: vec![
                {
                    let mut f = field("system_prompt", "System Prompt", "string");
                    f.description = Some("Base system prompt for the agent".into());
                    f
                },
                {
                    let mut f = field("max_turns", "Max Turns", "number");
                    f.description = Some("Maximum tool-use turns per request".into());
                    f.default_value = Some("50".into());
                    f
                },
            ],
        },
        ConfigSectionSchema {
            key: "agent.tools".into(),
            label: "Agent Tools".into(),
            description: Some("Enable/disable tool categories".into()),
            order: 25,
            icon: "wrench".into(),
            fields: vec![
                {
                    let mut f = field("filesystem", "Filesystem Tools", "boolean");
                    f.description = Some("Read, write, edit, glob, grep".into());
                    f.default_value = Some("true".into());
                    f
                },
                {
                    let mut f = field("execute", "Execute Command", "boolean");
                    f.description = Some("Shell command execution".into());
                    f.default_value = Some("true".into());
                    f
                },
            ],
        },
        ConfigSectionSchema {
            key: "memory".into(),
            label: "Memory".into(),
            description: Some("Long-term memory and session management".into()),
            order: 30,
            icon: "database".into(),
            fields: vec![
                {
                    let mut f = field("ltm_enabled", "Long-Term Memory", "boolean");
                    f.description = Some("Enable embedding-based long-term memory".into());
                    f.default_value = Some("false".into());
                    f
                },
                {
                    let mut f = field("auto_memorize", "Auto Memorize", "boolean");
                    f.description = Some("Automatically extract and store memories".into());
                    f.default_value = Some("false".into());
                    f
                },
            ],
        },
        ConfigSectionSchema {
            key: "context".into(),
            label: "Context".into(),
            description: Some("Context injection limits for workspace files".into()),
            order: 35,
            icon: "file-text".into(),
            fields: vec![
                {
                    let mut f = field("max_chars_per_file", "Max Chars Per File", "number");
                    f.description = Some("Truncation limit per context file (0=unlimited)".into());
                    f.default_value = Some("0".into());
                    f
                },
                {
                    let mut f = field("total_max_chars", "Total Max Chars", "number");
                    f.description =
                        Some("Total context budget across all files (0=unlimited)".into());
                    f.default_value = Some("0".into());
                    f
                },
            ],
        },
        ConfigSectionSchema {
            key: "session".into(),
            label: "Session".into(),
            description: Some("Session persistence and compaction".into()),
            order: 40,
            icon: "history".into(),
            fields: vec![
                {
                    let mut f = field("auto_compact", "Auto Compact", "boolean");
                    f.description = Some("Automatically compact long sessions".into());
                    f.default_value = Some("false".into());
                    f
                },
                {
                    let mut f = field("compact_threshold", "Compact Threshold", "number");
                    f.description = Some("Message count before auto-compaction triggers".into());
                    f.default_value = Some("50".into());
                    f
                },
            ],
        },
        ConfigSectionSchema {
            key: "serve".into(),
            label: "Web Server".into(),
            description: Some("Gateway web server settings".into()),
            order: 50,
            icon: "globe".into(),
            fields: vec![
                {
                    let mut f = field("port", "Port", "number");
                    f.description = Some("HTTP server port".into());
                    f.default_value = Some("3000".into());
                    f
                },
                {
                    let mut f = field("host", "Host", "string");
                    f.description = Some("Bind address".into());
                    f.placeholder = Some("0.0.0.0".into());
                    f.default_value = Some("0.0.0.0".into());
                    f
                },
                {
                    let mut f = field("cors_origins", "CORS Origins", "string");
                    f.description = Some("Allowed CORS origins (comma-separated)".into());
                    f.placeholder = Some("*".into());
                    f
                },
            ],
        },
        ConfigSectionSchema {
            key: "auth".into(),
            label: "Authentication".into(),
            description: Some("Gateway authentication and access control".into()),
            order: 55,
            icon: "shield".into(),
            fields: vec![
                {
                    let mut f = field("enabled", "Enabled", "boolean");
                    f.description = Some("Enable gateway authentication".into());
                    f.default_value = Some("false".into());
                    f
                },
                {
                    let mut f = field("token", "Auth Token", "secret");
                    f.description = Some("Bearer token for API access".into());
                    f.sensitive = true;
                    f
                },
                {
                    let mut f = field("token_env", "Token Env Var", "string");
                    f.description = Some("Environment variable for auth token".into());
                    f.placeholder = Some("SYNAPSE_AUTH_TOKEN".into());
                    f
                },
            ],
        },
        ConfigSectionSchema {
            key: "paths".into(),
            label: "Paths".into(),
            description: Some("File system paths for data storage".into()),
            order: 60,
            icon: "folder".into(),
            fields: vec![
                {
                    let mut f = field("sessions_dir", "Sessions Directory", "string");
                    f.description = Some("Directory for session transcripts".into());
                    f.default_value = Some(".sessions".into());
                    f
                },
                {
                    let mut f = field("memory_file", "Memory File", "string");
                    f.description = Some("Path for long-term memory storage".into());
                    f.default_value = Some("AGENTS.md".into());
                    f
                },
            ],
        },
        ConfigSectionSchema {
            key: "subagent".into(),
            label: "Sub-Agents".into(),
            description: Some("Sub-agent spawning configuration".into()),
            order: 70,
            icon: "users".into(),
            fields: vec![
                {
                    let mut f = field("enabled", "Enabled", "boolean");
                    f.description = Some("Allow agent to spawn sub-agents".into());
                    f.default_value = Some("true".into());
                    f
                },
                {
                    let mut f = field("max_depth", "Max Depth", "number");
                    f.description = Some("Maximum nesting depth for sub-agents".into());
                    f.default_value = Some("3".into());
                    f
                },
                {
                    let mut f = field("max_concurrent", "Max Concurrent", "number");
                    f.description = Some("Maximum concurrent sub-agents".into());
                    f.default_value = Some("5".into());
                    f
                },
                {
                    let mut f = field("timeout_secs", "Timeout (seconds)", "number");
                    f.description = Some("Sub-agent execution timeout".into());
                    f.default_value = Some("300".into());
                    f
                },
            ],
        },
        ConfigSectionSchema {
            key: "rate_limit".into(),
            label: "Rate Limiting".into(),
            description: Some("Model call rate limiting".into()),
            order: 75,
            icon: "gauge".into(),
            fields: vec![
                {
                    let mut f = field("requests_per_minute", "Requests/Min", "number");
                    f.description = Some("Maximum model requests per minute".into());
                    f
                },
                {
                    let mut f = field("tokens_per_minute", "Tokens/Min", "number");
                    f.description = Some("Maximum tokens per minute".into());
                    f
                },
            ],
        },
        ConfigSectionSchema {
            key: "security".into(),
            label: "Security".into(),
            description: Some("Security middleware (SSRF guard, secret masking)".into()),
            order: 80,
            icon: "lock".into(),
            fields: vec![
                {
                    let mut f = field("ssrf_guard", "SSRF Guard", "boolean");
                    f.description = Some("Block requests to private/internal IPs".into());
                    f.default_value = Some("true".into());
                    f
                },
                {
                    let mut f = field("secret_masking", "Secret Masking", "boolean");
                    f.description = Some("Mask sensitive values in logs and responses".into());
                    f.default_value = Some("true".into());
                    f
                },
            ],
        },
        ConfigSectionSchema {
            key: "heartbeat".into(),
            label: "Heartbeat".into(),
            description: Some("Periodic proactive agent execution".into()),
            order: 85,
            icon: "heart-pulse".into(),
            fields: vec![
                {
                    let mut f = field("enabled", "Enabled", "boolean");
                    f.description = Some("Enable periodic heartbeat runs".into());
                    f.default_value = Some("false".into());
                    f
                },
                {
                    let mut f = field("interval_secs", "Interval (seconds)", "number");
                    f.description = Some("Seconds between heartbeat runs".into());
                    f.default_value = Some("3600".into());
                    f
                },
            ],
        },
        ConfigSectionSchema {
            key: "reflection".into(),
            label: "Reflection".into(),
            description: Some("Post-session self-reflection for agent evolution".into()),
            order: 90,
            icon: "sparkles".into(),
            fields: vec![{
                let mut f = field("enabled", "Enabled", "boolean");
                f.description = Some("Enable post-session reflection".into());
                f.default_value = Some("false".into());
                f
            }],
        },
        ConfigSectionSchema {
            key: "logging".into(),
            label: "Logging".into(),
            description: Some("Console log output level".into()),
            order: 100,
            icon: "scroll-text".into(),
            fields: vec![{
                let mut f = field("level", "Console Log Level", "enum");
                f.description =
                    Some("Console output level (overridden by RUST_LOG env var)".into());
                f.options = Some(
                    vec!["trace", "debug", "info", "warn", "error", "off"]
                        .into_iter()
                        .map(String::from)
                        .collect(),
                );
                f.default_value = Some("info".into());
                f
            }],
        },
        ConfigSectionSchema {
            key: "logging.file".into(),
            label: "Logging \u{b7} File".into(),
            description: Some("File logging configuration (persistent logs)".into()),
            order: 101,
            icon: "scroll-text".into(),
            fields: vec![
                {
                    let mut f = field("enabled", "Enabled", "boolean");
                    f.description = Some("Write logs to files".into());
                    f.default_value = Some("true".into());
                    f
                },
                {
                    let mut f = field("path", "Log Directory", "string");
                    f.description = Some("Directory for log files (supports ~ expansion)".into());
                    f.default_value = Some("~/.synapse/logs".into());
                    f
                },
                {
                    let mut f = field("level", "File Log Level", "enum");
                    f.description =
                        Some("File log level (can be more verbose than console)".into());
                    f.options = Some(
                        vec!["trace", "debug", "info", "warn", "error"]
                            .into_iter()
                            .map(String::from)
                            .collect(),
                    );
                    f.default_value = Some("debug".into());
                    f
                },
                {
                    let mut f = field("format", "Format", "enum");
                    f.description = Some("Log file format".into());
                    f.options = Some(
                        vec!["json", "pretty"]
                            .into_iter()
                            .map(String::from)
                            .collect(),
                    );
                    f.default_value = Some("json".into());
                    f
                },
                {
                    let mut f = field("rotation", "Rotation", "enum");
                    f.description = Some("Log file rotation strategy".into());
                    f.options = Some(
                        vec!["daily", "hourly", "never"]
                            .into_iter()
                            .map(String::from)
                            .collect(),
                    );
                    f.default_value = Some("daily".into());
                    f
                },
                {
                    let mut f = field("max_days", "Max Retention Days", "number");
                    f.description = Some("Days to retain log files (0 = keep forever)".into());
                    f.default_value = Some("7".into());
                    f
                },
                {
                    let mut f = field("max_files", "Max Files", "number");
                    f.description = Some("Maximum number of log files to retain".into());
                    f.default_value = Some("30".into());
                    f
                },
            ],
        },
        ConfigSectionSchema {
            key: "logging.memory".into(),
            label: "Logging \u{b7} Memory Buffer".into(),
            description: Some("In-memory ring buffer for dashboard /api/logs queries".into()),
            order: 102,
            icon: "scroll-text".into(),
            fields: vec![
                {
                    let mut f = field("capacity", "Buffer Capacity", "number");
                    f.description = Some("Maximum entries in the ring buffer".into());
                    f.default_value = Some("10000".into());
                    f
                },
                {
                    let mut f = field("level", "Buffer Log Level", "enum");
                    f.description = Some("Minimum level for memory buffer capture".into());
                    f.options = Some(
                        vec!["trace", "debug", "info", "warn", "error"]
                            .into_iter()
                            .map(String::from)
                            .collect(),
                    );
                    f.default_value = Some("info".into());
                    f
                },
            ],
        },
        ConfigSectionSchema {
            key: "workspace".into(),
            label: "Workspace".into(),
            description: Some("Workspace directory for context files".into()),
            order: 105,
            icon: "folder-open".into(),
            fields: vec![{
                let mut f = field("workspace", "Workspace Path", "string");
                f.description =
                    Some("Path to workspace directory (default: ~/.synapse/workspace/)".into());
                f.placeholder = Some("~/.synapse/workspace/".into());
                f
            }],
        },
        ConfigSectionSchema {
            key: "docker".into(),
            label: "Docker Sandbox".into(),
            description: Some("Sandboxed command execution in Docker containers".into()),
            order: 110,
            icon: "lock".into(),
            fields: vec![
                {
                    let mut f = field("enabled", "Enabled", "boolean");
                    f.description = Some("Run tool commands inside Docker containers".into());
                    f.default_value = Some("false".into());
                    f
                },
                {
                    let mut f = field("image", "Image", "string");
                    f.description = Some("Docker image for sandbox".into());
                    f.placeholder = Some("ubuntu:22.04".into());
                    f
                },
                {
                    let mut f = field("memory_limit", "Memory Limit", "string");
                    f.description = Some("Container memory limit (e.g. 512m, 1g)".into());
                    f.placeholder = Some("512m".into());
                    f
                },
                {
                    let mut f = field("cpu_limit", "CPU Limit", "number");
                    f.description = Some("CPU core limit for container".into());
                    f
                },
                {
                    let mut f = field("network", "Network Access", "boolean");
                    f.description = Some("Allow network access in sandbox".into());
                    f.default_value = Some("false".into());
                    f
                },
            ],
        },
        ConfigSectionSchema {
            key: "voice".into(),
            label: "Voice".into(),
            description: Some("Text-to-speech and speech-to-text configuration".into()),
            order: 115,
            icon: "bot".into(),
            fields: vec![
                {
                    let mut f = field("tts_provider", "TTS Provider", "string");
                    f.description = Some("Text-to-speech provider".into());
                    f.placeholder = Some("openai".into());
                    f
                },
                {
                    let mut f = field("stt_provider", "STT Provider", "string");
                    f.description = Some("Speech-to-text provider".into());
                    f.placeholder = Some("openai".into());
                    f
                },
                {
                    let mut f = field("voice", "Voice", "string");
                    f.description = Some("Voice name/ID for TTS".into());
                    f.placeholder = Some("alloy".into());
                    f
                },
                {
                    let mut f = field("api_key_env", "API Key Env Var", "string");
                    f.description = Some("Environment variable for voice API key".into());
                    f
                },
            ],
        },
        ConfigSectionSchema {
            key: "secrets".into(),
            label: "Secret Masking".into(),
            description: Some("Mask sensitive values in logs and responses".into()),
            order: 82,
            icon: "lock".into(),
            fields: vec![{
                let mut f = field("mask_api_keys", "Mask API Keys", "boolean");
                f.description = Some("Automatically mask API keys in output".into());
                f.default_value = Some("true".into());
                f
            }],
        },
        ConfigSectionSchema {
            key: "tool_policy".into(),
            label: "Tool Policy".into(),
            description: Some(
                "Tool access control \u{2014} allow/deny lists and owner-only tools".into(),
            ),
            order: 78,
            icon: "shield".into(),
            fields: vec![],
        },
        ConfigSectionSchema {
            key: "gateway".into(),
            label: "Gateway Deployment".into(),
            description: Some("Multi-gateway deployment and leader election".into()),
            order: 120,
            icon: "globe".into(),
            fields: vec![
                {
                    let mut f = field("instance_id", "Instance ID", "string");
                    f.description = Some("Unique identifier for this gateway instance".into());
                    f
                },
                {
                    let mut f = field("shared_store_url", "Shared Store URL", "string");
                    f.description = Some("URL for shared state store (e.g. Redis)".into());
                    f
                },
                {
                    let mut f = field("leader_election", "Leader Election", "boolean");
                    f.description = Some("Enable leader election among gateway instances".into());
                    f.default_value = Some("false".into());
                    f
                },
            ],
        },
        ConfigSectionSchema {
            key: "hub".into(),
            label: "ClawHub Registry".into(),
            description: Some("ClawHub registry for sharing agents and skills".into()),
            order: 125,
            icon: "globe".into(),
            fields: vec![
                {
                    let mut f = field("url", "Hub URL", "string");
                    f.description = Some("ClawHub registry endpoint".into());
                    f
                },
                {
                    let mut f = field("api_key_env", "API Key Env Var", "string");
                    f.description = Some("Environment variable for hub API key".into());
                    f
                },
            ],
        },
        // Bot channel sections
        ConfigSectionSchema {
            key: "lark".into(),
            label: "Lark".into(),
            description: Some("Lark bot platform credentials".into()),
            order: 200,
            icon: "bot".into(),
            fields: vec![
                {
                    let mut f = field("app_id", "App ID", "string");
                    f.description = Some("Lark app ID from developer console".into());
                    f
                },
                {
                    let mut f = field("app_secret", "App Secret", "secret");
                    f.description = Some("Lark app secret".into());
                    f.sensitive = true;
                    f
                },
                {
                    let mut f = field("verification_token", "Verification Token", "secret");
                    f.description = Some("Event subscription verification token".into());
                    f.sensitive = true;
                    f
                },
                {
                    let mut f = field("encrypt_key", "Encrypt Key", "secret");
                    f.description = Some("Event encryption key".into());
                    f.sensitive = true;
                    f
                },
            ],
        },
        ConfigSectionSchema {
            key: "slack".into(),
            label: "Slack".into(),
            description: Some("Slack bot credentials".into()),
            order: 201,
            icon: "bot".into(),
            fields: vec![
                {
                    let mut f = field("bot_token", "Bot Token", "secret");
                    f.description = Some("Slack bot OAuth token (xoxb-...)".into());
                    f.sensitive = true;
                    f
                },
                {
                    let mut f = field("app_token", "App Token", "secret");
                    f.description = Some("Slack app-level token for Socket Mode (xapp-...)".into());
                    f.sensitive = true;
                    f
                },
                {
                    let mut f = field("signing_secret", "Signing Secret", "secret");
                    f.description = Some("Request verification signing secret".into());
                    f.sensitive = true;
                    f
                },
            ],
        },
        ConfigSectionSchema {
            key: "telegram".into(),
            label: "Telegram".into(),
            description: Some("Telegram bot credentials".into()),
            order: 202,
            icon: "bot".into(),
            fields: vec![{
                let mut f = field("bot_token", "Bot Token", "secret");
                f.description = Some("Telegram bot token from @BotFather".into());
                f.sensitive = true;
                f
            }],
        },
        ConfigSectionSchema {
            key: "discord".into(),
            label: "Discord".into(),
            description: Some("Discord bot credentials".into()),
            order: 203,
            icon: "bot".into(),
            fields: vec![
                {
                    let mut f = field("bot_token", "Bot Token", "secret");
                    f.description = Some("Discord bot token".into());
                    f.sensitive = true;
                    f
                },
                {
                    let mut f = field("application_id", "Application ID", "string");
                    f.description = Some("Discord application ID".into());
                    f
                },
            ],
        },
    ]
}

async fn get_config_schema(State(_state): State<AppState>) -> Json<ConfigSchemaResponse> {
    Json(ConfigSchemaResponse {
        sections: build_config_schema(),
        sensitive_patterns: vec![
            "api_key".into(),
            "token".into(),
            "secret".into(),
            "password".into(),
            "app_secret".into(),
            "signing_secret".into(),
            "bot_token".into(),
            "webhook_secret".into(),
        ],
    })
}
