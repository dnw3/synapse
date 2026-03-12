use std::collections::HashMap;
use std::path::{Path, PathBuf};

use axum::extract::{self, Query, State};
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::{delete, get, patch, post, put};
use axum::Router;
use serde::{Deserialize, Serialize};
use synaptic::core::MemoryStore;

use tracing;

use crate::gateway::state::AppState;

// ---------------------------------------------------------------------------
// Session overrides storage (file-based)
// ---------------------------------------------------------------------------

const SESSION_OVERRIDES_FILE: &str = "data/session_overrides.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SessionOverride {
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    verbose: Option<String>,
}

type SessionOverrides = HashMap<String, SessionOverride>;

fn overrides_path() -> PathBuf {
    PathBuf::from(SESSION_OVERRIDES_FILE)
}

fn load_overrides() -> SessionOverrides {
    let path = overrides_path();
    if !path.exists() {
        return SessionOverrides::new();
    }
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => SessionOverrides::new(),
    }
}

fn save_overrides(overrides: &SessionOverrides) -> Result<(), String> {
    let path = overrides_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(overrides).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn routes() -> Router<AppState> {
    Router::new()
        // Core
        .route("/dashboard/stats", get(get_stats))
        .route("/dashboard/usage", get(get_usage))
        .route("/dashboard/providers", get(get_providers))
        .route("/dashboard/health", get(get_health))
        .route("/dashboard/sessions", get(get_sessions))
        .route("/dashboard/schedules", get(get_schedules))
        .route("/dashboard/config", get(get_config))
        .route("/dashboard/config", put(put_config))
        .route("/dashboard/channels", get(get_channels))
        .route("/dashboard/skills", get(get_skills))
        .route("/dashboard/mcp", get(get_mcp))
        .route("/dashboard/requests", get(get_requests))
        .route("/dashboard/logs", get(get_logs))
        // Phase 1: Usage analytics
        .route("/dashboard/usage/timeseries", get(get_usage_timeseries))
        .route("/dashboard/usage/sessions", get(get_usage_sessions))
        // Phase 2: Schedules CRUD
        .route("/dashboard/schedules", post(create_schedule))
        .route("/dashboard/schedules/{name}", put(update_schedule))
        .route("/dashboard/schedules/{name}", delete(delete_schedule))
        .route(
            "/dashboard/schedules/{name}/trigger",
            post(trigger_schedule),
        )
        .route("/dashboard/schedules/{name}/toggle", post(toggle_schedule))
        .route("/dashboard/schedules/{name}/runs", get(get_schedule_runs))
        // Phase 3: Config advanced
        .route("/dashboard/config/schema", get(get_config_schema))
        .route("/dashboard/config/validate", post(validate_config))
        .route("/dashboard/config/reload", post(reload_config))
        // Phase 4: Agents
        .route("/dashboard/agents", get(get_agents))
        .route("/dashboard/agents", post(create_agent))
        .route("/dashboard/agents/{name}", put(update_agent))
        .route("/dashboard/agents/{name}", delete(delete_agent))
        .route("/dashboard/tools", get(get_tools_catalog))
        // Phase 5: Sessions CRUD
        .route("/dashboard/sessions/{id}", delete(delete_session))
        .route("/dashboard/sessions/{id}", patch(patch_session))
        .route("/dashboard/sessions/{id}/compact", post(compact_session))
        // Phase 6: Channels toggle + config
        .route("/dashboard/channels/{name}/toggle", post(toggle_channel))
        .route("/dashboard/channels/{name}/config", put(put_channel_config))
        // Phase 7: Skills toggle
        .route("/dashboard/skills/{name}/toggle", post(toggle_skill))
        .route("/dashboard/skills/content", get(get_skill_content))
        .route("/dashboard/skills/files", get(get_skill_files))
        // Phase 8: Debug
        .route("/dashboard/debug/invoke", post(debug_invoke))
        .route("/dashboard/debug/health", get(debug_health))
        // Phase 9: Logs
        .route("/dashboard/logs/export", get(export_logs))
        // Phase 10: Version
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
        .route("/dashboard/identity", get(get_identity))
        // Skill Store (ClawHub etc.)
        .route("/dashboard/store/search", get(store_search))
        .route("/dashboard/store/skills", get(store_list))
        .route("/dashboard/store/skills/{slug}", get(store_detail))
        .route(
            "/dashboard/store/skills/{slug}/files",
            get(store_skill_files),
        )
        .route(
            "/dashboard/store/skills/{slug}/files/{*path}",
            get(store_skill_file_content),
        )
        .route("/dashboard/store/install", post(store_install))
        .route("/dashboard/store/status", get(store_status))
}

// ---------------------------------------------------------------------------
// 1. GET /api/dashboard/stats
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct StatsResponse {
    session_count: usize,
    total_messages: usize,
    total_input_tokens: u64,
    total_output_tokens: u64,
    total_cost_usd: f64,
    uptime_secs: u64,
    active_ws_sessions: usize,
}

async fn get_stats(
    State(state): State<AppState>,
) -> Result<Json<StatsResponse>, (StatusCode, String)> {
    let sessions = state
        .sessions
        .list_sessions()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let memory = state.sessions.memory();
    let mut total_messages = 0usize;
    for s in &sessions {
        total_messages += memory.load(&s.id).await.map(|m| m.len()).unwrap_or(0);
    }

    let snapshot = state.cost_tracker.snapshot().await;
    let active_ws = state.cancel_tokens.read().await.len();

    Ok(Json(StatsResponse {
        session_count: sessions.len(),
        total_messages,
        total_input_tokens: snapshot.total_input_tokens,
        total_output_tokens: snapshot.total_output_tokens,
        total_cost_usd: snapshot.estimated_cost_usd,
        uptime_secs: state.started_at.elapsed().as_secs(),
        active_ws_sessions: active_ws,
    }))
}

// ---------------------------------------------------------------------------
// 2. GET /api/dashboard/usage
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct UsageResponse {
    total_input_tokens: u64,
    total_output_tokens: u64,
    total_requests: u64,
    total_cost_usd: f64,
    per_model: Vec<ModelUsageEntry>,
}

#[derive(Serialize)]
struct ModelUsageEntry {
    model: String,
    input_tokens: u64,
    output_tokens: u64,
    requests: u64,
    cost_usd: f64,
}

async fn get_usage(State(state): State<AppState>) -> Json<UsageResponse> {
    let snapshot = state.cost_tracker.snapshot().await;

    let mut per_model: Vec<ModelUsageEntry> = snapshot
        .per_model
        .into_iter()
        .map(|(model, usage)| ModelUsageEntry {
            model,
            input_tokens: usage.input_tokens,
            output_tokens: usage.output_tokens,
            requests: usage.requests,
            cost_usd: usage.cost_usd,
        })
        .collect();
    per_model.sort_by(|a, b| a.model.cmp(&b.model));

    Json(UsageResponse {
        total_input_tokens: snapshot.total_input_tokens,
        total_output_tokens: snapshot.total_output_tokens,
        total_requests: snapshot.total_requests,
        total_cost_usd: snapshot.estimated_cost_usd,
        per_model,
    })
}

// ---------------------------------------------------------------------------
// Phase 1: GET /api/dashboard/usage/timeseries
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct TimeseriesQuery {
    #[allow(dead_code)]
    from: Option<String>,
    #[allow(dead_code)]
    to: Option<String>,
    #[allow(dead_code)]
    granularity: Option<String>,
}

#[derive(Serialize)]
struct TimeseriesEntry {
    timestamp: String,
    input_tokens: u64,
    output_tokens: u64,
    cost: f64,
    count: u64,
}

async fn get_usage_timeseries(
    State(state): State<AppState>,
    Query(_query): Query<TimeseriesQuery>,
) -> Json<Vec<TimeseriesEntry>> {
    // Build synthetic timeseries from per-model aggregates
    // In a full implementation, this would read from a persistent usage store
    let snapshot = state.cost_tracker.snapshot().await;
    let now = chrono::Utc::now();

    // Generate a single "today" bucket from current totals
    let entries = if snapshot.total_requests > 0 {
        vec![TimeseriesEntry {
            timestamp: now.format("%Y-%m-%dT%H:00:00Z").to_string(),
            input_tokens: snapshot.total_input_tokens,
            output_tokens: snapshot.total_output_tokens,
            cost: snapshot.estimated_cost_usd,
            count: snapshot.total_requests,
        }]
    } else {
        vec![]
    };

    Json(entries)
}

// ---------------------------------------------------------------------------
// Phase 1: GET /api/dashboard/usage/sessions
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct UsageSessionsQuery {
    #[allow(dead_code)]
    from: Option<String>,
    #[allow(dead_code)]
    to: Option<String>,
    #[allow(dead_code)]
    sort: Option<String>,
    #[allow(dead_code)]
    limit: Option<usize>,
    #[allow(dead_code)]
    offset: Option<usize>,
}

#[derive(Serialize)]
struct UsageSessionEntry {
    session_id: String,
    input_tokens: u64,
    output_tokens: u64,
    cost: f64,
    request_count: u64,
}

async fn get_usage_sessions(
    State(state): State<AppState>,
    Query(_query): Query<UsageSessionsQuery>,
) -> Result<Json<Vec<UsageSessionEntry>>, (StatusCode, String)> {
    let sessions = state
        .sessions
        .list_sessions()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let memory = state.sessions.memory();
    let mut result = Vec::with_capacity(sessions.len());

    for s in sessions {
        // Count messages in this session
        let msg_count = memory
            .load(&s.id)
            .await
            .map(|msgs| msgs.len() as u64)
            .unwrap_or(0);

        // Approximate input/output split (60/40 is typical for chat)
        let input_tokens = (s.token_count as f64 * 0.6) as u64;
        let output_tokens = s.token_count.saturating_sub(input_tokens);

        result.push(UsageSessionEntry {
            session_id: s.id,
            input_tokens,
            output_tokens,
            cost: 0.0,                    // per-session cost tracking not yet available
            request_count: msg_count / 2, // each user message ≈ 1 request
        });
    }

    Ok(Json(result))
}

// ---------------------------------------------------------------------------
// 3. GET /api/dashboard/providers
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct ProviderResponse {
    name: String,
    base_url: String,
    models: Vec<String>,
}

async fn get_providers(State(state): State<AppState>) -> Json<Vec<ProviderResponse>> {
    let config = &state.config;

    let mut provider_models: HashMap<String, Vec<String>> = HashMap::new();
    if let Some(models) = &config.model_catalog {
        for m in models {
            let provider = m.provider.clone().unwrap_or_else(|| "default".to_string());
            provider_models
                .entry(provider)
                .or_default()
                .push(m.name.clone());
        }
    }

    let mut providers = Vec::new();
    if let Some(catalog) = &config.provider_catalog {
        for p in catalog {
            let models = provider_models.remove(&p.name).unwrap_or_default();
            providers.push(ProviderResponse {
                name: p.name.clone(),
                base_url: p.base_url.clone(),
                models,
            });
        }
    }

    if !providers.iter().any(|p| p.name == "default") {
        let base_model = state.config.base.model.model.clone();
        let base_provider = state.config.base.model.provider.clone();
        providers.insert(
            0,
            ProviderResponse {
                name: base_provider,
                base_url: state.config.base.model.base_url.clone().unwrap_or_default(),
                models: vec![base_model],
            },
        );
    }

    Json(providers)
}

// ---------------------------------------------------------------------------
// 4. GET /api/dashboard/health
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    uptime_secs: u64,
    auth_enabled: bool,
    memory_entries: usize,
    active_sessions: usize,
    session_count: usize,
    config_summary: ConfigSummary,
}

#[derive(Serialize)]
struct ConfigSummary {
    model: String,
    provider: String,
    mcp_servers: usize,
    scheduled_jobs: usize,
    bot_channels: usize,
}

async fn get_health(
    State(state): State<AppState>,
) -> Result<Json<HealthResponse>, (StatusCode, String)> {
    let sessions = state
        .sessions
        .list_sessions()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let memory = state.sessions.memory();
    let mut memory_entries = 0usize;
    for s in &sessions {
        memory_entries += memory.load(&s.id).await.map(|m| m.len()).unwrap_or(0);
    }

    let active = state.cancel_tokens.read().await.len();
    let auth_enabled = state
        .auth
        .as_ref()
        .map(|a| a.config.enabled)
        .unwrap_or(false);

    let bot_channels = count_bot_channels(&state.config);

    Ok(Json(HealthResponse {
        status: "ok".to_string(),
        uptime_secs: state.started_at.elapsed().as_secs(),
        auth_enabled,
        memory_entries,
        active_sessions: active,
        session_count: sessions.len(),
        config_summary: ConfigSummary {
            model: state.config.base.model.model.clone(),
            provider: state.config.base.model.provider.clone(),
            mcp_servers: state.config.base.mcp.as_ref().map(|m| m.len()).unwrap_or(0),
            scheduled_jobs: state
                .config
                .schedules
                .as_ref()
                .map(|s| s.len())
                .unwrap_or(0),
            bot_channels,
        },
    }))
}

// ---------------------------------------------------------------------------
// 5. GET /api/dashboard/sessions
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct SessionResponse {
    id: String,
    created_at: String,
    message_count: usize,
    token_count: u64,
    compaction_count: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking_level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    verbose_level: Option<String>,
}

async fn get_sessions(
    State(state): State<AppState>,
) -> Result<Json<Vec<SessionResponse>>, (StatusCode, String)> {
    let sessions = state
        .sessions
        .list_sessions()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let overrides = load_overrides();
    let memory = state.sessions.memory();
    let mut result = Vec::with_capacity(sessions.len());
    for s in sessions {
        let messages = memory.load(&s.id).await.unwrap_or_default();
        let count = messages.len();
        // Derive title from first human message (truncated to 60 chars)
        let title = messages.iter().find(|m| m.is_human()).map(|m| {
            let content = m.content();
            if content.chars().count() > 60 {
                format!("{}...", content.chars().take(60).collect::<String>())
            } else {
                content.to_string()
            }
        });
        let ovr = overrides.get(&s.id);
        result.push(SessionResponse {
            id: s.id,
            created_at: parse_system_time_string(&s.created_at),
            message_count: count,
            token_count: s.token_count,
            compaction_count: s.compaction_count,
            title,
            label: ovr.and_then(|o| o.label.clone()),
            thinking_level: ovr.and_then(|o| o.thinking.clone()),
            verbose_level: ovr.and_then(|o| o.verbose.clone()),
        });
    }

    Ok(Json(result))
}

// ---------------------------------------------------------------------------
// Phase 5: DELETE /api/dashboard/sessions/{id}
// ---------------------------------------------------------------------------

async fn delete_session(
    State(state): State<AppState>,
    extract::Path(id): extract::Path<String>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    state
        .sessions
        .delete_session(&id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(OkResponse { ok: true }))
}

// ---------------------------------------------------------------------------
// Phase 5: PATCH /api/dashboard/sessions/{id}
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct PatchSessionRequest {
    display_name: Option<String>,
    label: Option<String>,
    thinking: Option<String>,
    verbose: Option<String>,
}

async fn patch_session(
    State(_state): State<AppState>,
    extract::Path(id): extract::Path<String>,
    Json(body): Json<PatchSessionRequest>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    let mut overrides = load_overrides();
    let entry = overrides.entry(id).or_default();

    // Apply label (from either field)
    if let Some(label) = body.label.or(body.display_name) {
        if label.is_empty() {
            entry.label = None;
        } else {
            entry.label = Some(label);
        }
    }
    if let Some(thinking) = body.thinking {
        entry.thinking = Some(thinking);
    }
    if let Some(verbose) = body.verbose {
        entry.verbose = Some(verbose);
    }

    save_overrides(&overrides).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(OkResponse { ok: true }))
}

// ---------------------------------------------------------------------------
// Phase 5: POST /api/dashboard/sessions/{id}/compact
// ---------------------------------------------------------------------------

async fn compact_session(
    State(_state): State<AppState>,
    extract::Path(_id): extract::Path<String>,
) -> Json<OkResponse> {
    // Compaction trigger — would need condenser integration
    Json(OkResponse { ok: true })
}

// ---------------------------------------------------------------------------
// 6. GET /api/dashboard/schedules
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
// Phase 2: POST /api/dashboard/schedules
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
    // Read, modify, and write back config
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
// Phase 2: PUT /api/dashboard/schedules/{name}
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
// Phase 2: DELETE /api/dashboard/schedules/{name}
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

    // Ensure log directory exists
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
// Phase 2: POST /api/dashboard/schedules/{name}/trigger
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
// Phase 2: GET /api/dashboard/schedules/{name}/runs
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
// Phase 2: POST /api/dashboard/schedules/{name}/toggle
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct ToggleResponse {
    enabled: bool,
}

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
// 7. GET /api/dashboard/config
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
// 8. PUT /api/dashboard/config
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
// Phase 3: POST /api/dashboard/config/validate
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
    // First: TOML syntax check
    let toml_val = match toml::from_str::<toml::Value>(&body.content) {
        Ok(v) => v,
        Err(e) => {
            return Json(ValidateConfigResponse {
                valid: false,
                errors: vec![format!("TOML syntax: {}", e)],
            });
        }
    };

    // Second: structural validation — try to deserialize into SynapseConfig
    let mut errors = Vec::new();
    match toml::from_str::<crate::config::SynapseConfig>(&body.content) {
        Ok(_) => {}
        Err(e) => {
            errors.push(format!("Config structure: {}", e));
        }
    }

    // Third: warn about sensitive fields in clear text
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
// Phase 3: POST /api/dashboard/config/reload
// ---------------------------------------------------------------------------

async fn reload_config(State(_state): State<AppState>) -> Json<OkResponse> {
    // Config reload would require AppState to hold a mutable config reference
    // Placeholder for now
    Json(OkResponse { ok: true })
}

// ---------------------------------------------------------------------------
// Phase 3: GET /api/dashboard/config/schema
// ---------------------------------------------------------------------------

#[derive(Serialize, Clone)]
struct ConfigFieldSchema {
    key: String,
    label: String,
    #[serde(rename = "type")]
    field_type: String, // "string" | "number" | "boolean" | "enum" | "array" | "secret"
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
            label: "Logging · File".into(),
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
            label: "Logging · Memory Buffer".into(),
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
        // --- Missing config sections ---
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
            description: Some("Tool access control — allow/deny lists and owner-only tools".into()),
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
            label: "Lark / Feishu".into(),
            description: Some("Lark (Feishu) bot platform credentials".into()),
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
        // Channel overrides
        ConfigSectionSchema {
            key: "channel_overrides.lark".into(),
            label: "Lark Overrides".into(),
            description: Some("Per-channel behavior overrides for Lark".into()),
            order: 300,
            icon: "settings".into(),
            fields: vec![{
                let mut f = field("enabled", "Enabled", "boolean");
                f.description = Some("Enable/disable Lark channel".into());
                f.default_value = Some("false".into());
                f
            }],
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

// ---------------------------------------------------------------------------
// 9. GET /api/dashboard/channels
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct ChannelResponse {
    name: String,
    enabled: bool,
    config: HashMap<String, String>,
}

/// Extract config fields for a channel from the raw TOML value.
fn extract_channel_config(toml_val: &toml::Value, channel_name: &str) -> HashMap<String, String> {
    let table = toml_val
        .as_table()
        .and_then(|root| root.get(channel_name))
        .and_then(|v| v.as_table());
    let Some(table) = table else {
        return HashMap::new();
    };
    table
        .iter()
        .filter_map(|(k, v)| {
            let s = match v {
                toml::Value::String(s) => s.clone(),
                toml::Value::Boolean(b) => b.to_string(),
                toml::Value::Integer(i) => i.to_string(),
                toml::Value::Float(f) => f.to_string(),
                _ => return None,
            };
            Some((k.clone(), s))
        })
        .collect()
}

async fn get_channels(State(state): State<AppState>) -> Json<Vec<ChannelResponse>> {
    let config = &state.config;

    // Read TOML to extract per-channel config fields AND channel_overrides
    let toml_val: toml::Value = read_config_file()
        .await
        .ok()
        .and_then(|(_, content)| toml::from_str(&content).ok())
        .unwrap_or(toml::Value::Table(Default::default()));

    // Helper: check if a channel is enabled by reading channel_overrides from TOML first,
    // falling back to in-memory config (section presence at startup).
    let resolve_enabled = |name: &str, startup_exists: bool| -> bool {
        toml_val
            .get("channel_overrides")
            .and_then(|o| o.get(name))
            .and_then(|c| c.get("enabled"))
            .and_then(|v| v.as_bool())
            .unwrap_or(startup_exists)
    };

    let channels = vec![
        ("lark", config.lark.is_some()),
        ("slack", config.slack.is_some()),
        ("telegram", config.telegram.is_some()),
        ("discord", config.discord.is_some()),
        ("dingtalk", config.dingtalk.is_some()),
        ("mattermost", config.mattermost.is_some()),
        ("matrix", config.matrix.is_some()),
        ("whatsapp", config.whatsapp.is_some()),
        ("teams", config.teams.is_some()),
        ("signal", config.signal.is_some()),
        ("wechat", config.wechat.is_some()),
        ("imessage", config.imessage.is_some()),
        ("line", config.line.is_some()),
        ("googlechat", config.googlechat.is_some()),
        ("irc", config.irc.is_some()),
        ("webchat", config.webchat.is_some()),
        ("twitch", config.twitch.is_some()),
        ("nostr", config.nostr.is_some()),
        ("nextcloud", config.nextcloud.is_some()),
        ("synology", config.synology.is_some()),
        ("tlon", config.tlon.is_some()),
        ("zalo", config.zalo.is_some()),
    ];

    Json(
        channels
            .into_iter()
            .map(|(name, startup_exists)| ChannelResponse {
                config: extract_channel_config(&toml_val, name),
                enabled: resolve_enabled(name, startup_exists),
                name: name.to_string(),
            })
            .collect(),
    )
}

// ---------------------------------------------------------------------------
// Phase 6: POST /api/dashboard/channels/{name}/toggle
// ---------------------------------------------------------------------------

async fn toggle_channel(
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

    // Determine current enabled state: prefer channel_overrides from TOML,
    // then check if channel section exists in TOML (not in-memory config).
    let known_channels = [
        "lark",
        "slack",
        "telegram",
        "discord",
        "dingtalk",
        "mattermost",
        "matrix",
        "whatsapp",
        "teams",
        "signal",
        "wechat",
        "imessage",
        "line",
        "googlechat",
        "irc",
        "webchat",
        "twitch",
        "nostr",
        "nextcloud",
        "synology",
        "tlon",
        "zalo",
    ];
    if !known_channels.contains(&name.as_str()) {
        return Err((StatusCode::NOT_FOUND, format!("unknown channel '{}'", name)));
    }
    // Check if channel section exists in TOML on disk (not just in-memory startup state)
    let section_exists = doc
        .get(&name)
        .and_then(|v| v.as_table())
        .map(|t| !t.is_empty())
        .unwrap_or(false);

    // Check existing override in TOML (may differ from in-memory state)
    let current_override = doc
        .get("channel_overrides")
        .and_then(|o| o.get(&name))
        .and_then(|c| c.get("enabled"))
        .and_then(|v| v.as_bool());
    let current_enabled = current_override.unwrap_or(section_exists);
    let new_enabled = !current_enabled;

    // Persist to channel_overrides section in TOML
    let root = doc.as_table_mut().unwrap();
    let overrides = root
        .entry("channel_overrides")
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));

    if let toml::Value::Table(tbl) = overrides {
        let ch_entry = tbl
            .entry(&name)
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
        if let toml::Value::Table(ch_tbl) = ch_entry {
            ch_tbl.insert("enabled".to_string(), toml::Value::Boolean(new_enabled));
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

    let enabled = new_enabled;
    tracing::info!(channel = %name, enabled, "channel toggled");

    Ok(Json(ToggleResponse {
        enabled: new_enabled,
    }))
}

// ---------------------------------------------------------------------------
// Phase 6b: PUT /api/dashboard/channels/{name}/config
// ---------------------------------------------------------------------------

async fn put_channel_config(
    State(_state): State<AppState>,
    extract::Path(name): extract::Path<String>,
    Json(body): Json<HashMap<String, String>>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    let path = config_file_path();
    let content = tokio::fs::read_to_string(&path).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("read failed: {}", e),
        )
    })?;

    let mut doc: toml::Value = toml::from_str(&content).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("parse failed: {}", e),
        )
    })?;

    // Ensure root is a table
    let root = doc.as_table_mut().ok_or_else(|| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "config root is not a table".to_string(),
        )
    })?;

    // Get or create the channel section
    if !root.contains_key(&name) {
        root.insert(name.clone(), toml::Value::Table(Default::default()));
    }
    let section = root
        .get_mut(&name)
        .and_then(|v| v.as_table_mut())
        .ok_or_else(|| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "channel section is not a table".to_string(),
            )
        })?;

    // Update fields
    for (key, value) in body {
        if value.is_empty() {
            section.remove(&key);
        } else {
            section.insert(key, toml::Value::String(value));
        }
    }

    let new_content = toml::to_string_pretty(&doc).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("serialize failed: {}", e),
        )
    })?;
    tokio::fs::write(&path, &new_content).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("write failed: {}", e),
        )
    })?;

    tracing::info!(channel = %name, "channel config saved");

    Ok(Json(OkResponse { ok: true }))
}

// ---------------------------------------------------------------------------
// 10. GET /api/dashboard/skills
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct SkillResponse {
    name: String,
    path: String,
    source: String,
    description: String,
    user_invocable: bool,
    enabled: bool,
    eligible: bool,
    emoji: Option<String>,
    homepage: Option<String>,
    version: Option<String>,
    missing_env: Vec<String>,
    missing_bins: Vec<String>,
    has_install_specs: bool,
}

async fn get_skills(
    State(state): State<AppState>,
) -> Result<Json<Vec<SkillResponse>>, (StatusCode, String)> {
    let mut skills = Vec::new();

    let dirs: Vec<(&str, String)> = {
        let mut v = vec![("project", ".claude/skills".to_string())];
        if let Some(home) = dirs::home_dir() {
            // Synapse personal (highest priority)
            v.push((
                "personal",
                home.join(".synapse/skills").to_string_lossy().to_string(),
            ));
            // OpenClaw compat personal
            v.push((
                "personal",
                home.join(".claude/skills").to_string_lossy().to_string(),
            ));
        }
        v
    };

    // Dedup: higher-priority dirs override lower-priority ones by skill name
    let mut seen_names = std::collections::HashSet::new();

    for (source, dir_path) in dirs {
        if dir_path.is_empty() {
            continue;
        }
        let dir = Path::new(&dir_path);
        if !dir.exists() {
            continue;
        }
        if let Ok(mut entries) = tokio::fs::read_dir(dir).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("md") {
                    let name = path
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();

                    if !seen_names.insert(name.clone()) {
                        continue; // already registered from higher-priority dir
                    }

                    // Parse frontmatter for description and user_invocable
                    let (
                        description,
                        user_invocable,
                        eligible,
                        emoji,
                        homepage,
                        version,
                        missing_env,
                        missing_bins,
                        has_install,
                    ) = parse_skill_full_info(&path).await;

                    let enabled = !state
                        .config
                        .skill_overrides
                        .get(&name)
                        .map(|o| !o.enabled)
                        .unwrap_or(false);

                    skills.push(SkillResponse {
                        name,
                        path: path.to_string_lossy().to_string(),
                        source: source.to_string(),
                        description,
                        user_invocable,
                        enabled,
                        eligible,
                        emoji,
                        homepage,
                        version,
                        missing_env,
                        missing_bins,
                        has_install_specs: has_install,
                    });
                }
            }

            // Also scan subdirectories for SKILL.md
            if let Ok(mut sub_entries) = tokio::fs::read_dir(dir).await {
                while let Ok(Some(sub_entry)) = sub_entries.next_entry().await {
                    let sub_path = sub_entry.path();
                    if !sub_path.is_dir() {
                        continue;
                    }
                    let skill_md = sub_path.join("SKILL.md");
                    if !skill_md.exists() {
                        continue;
                    }

                    let name = sub_path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("unknown")
                        .to_string();

                    if !seen_names.insert(name.clone()) {
                        continue; // already registered from higher-priority dir
                    }

                    let (
                        description,
                        user_invocable,
                        eligible,
                        emoji,
                        homepage,
                        version,
                        missing_env,
                        missing_bins,
                        has_install,
                    ) = parse_skill_full_info(&skill_md).await;

                    let enabled = !state
                        .config
                        .skill_overrides
                        .get(&name)
                        .map(|o| !o.enabled)
                        .unwrap_or(false);

                    skills.push(SkillResponse {
                        name,
                        path: skill_md.to_string_lossy().to_string(),
                        source: source.to_string(),
                        description,
                        user_invocable,
                        enabled,
                        eligible,
                        emoji,
                        homepage,
                        version,
                        missing_env,
                        missing_bins,
                        has_install_specs: has_install,
                    });
                }
            }
        }
    }

    Ok(Json(skills))
}

// ---------------------------------------------------------------------------
// Phase 7: POST /api/dashboard/skills/{name}/toggle
// ---------------------------------------------------------------------------

async fn toggle_skill(
    State(state): State<AppState>,
    extract::Path(name): extract::Path<String>,
) -> Result<Json<ToggleResponse>, (StatusCode, String)> {
    let (path, content) = read_config_file().await?;
    let mut doc: toml::Value = toml::from_str(&content).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("parse TOML: {}", e),
        )
    })?;

    // Determine current enabled state from in-memory config
    let current_enabled = !state
        .config
        .skill_overrides
        .get(&name)
        .map(|o| !o.enabled)
        .unwrap_or(false);
    let new_enabled = !current_enabled;

    // Ensure skill_overrides table exists and update the skill entry
    let root = doc.as_table_mut().unwrap();
    let overrides = root
        .entry("skill_overrides")
        .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));

    if let toml::Value::Table(tbl) = overrides {
        let skill_entry = tbl
            .entry(&name)
            .or_insert_with(|| toml::Value::Table(toml::map::Map::new()));
        if let toml::Value::Table(skill_tbl) = skill_entry {
            skill_tbl.insert("enabled".to_string(), toml::Value::Boolean(new_enabled));
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
// GET /api/dashboard/skills/content — Read skill file content
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct SkillContentQuery {
    path: String,
}

#[derive(Serialize)]
struct SkillContentResponse {
    content: String,
}

async fn get_skill_content(
    Query(query): Query<SkillContentQuery>,
) -> Result<Json<SkillContentResponse>, (StatusCode, String)> {
    let path = Path::new(&query.path);

    // Prevent path traversal before any other checks
    let path_str = path.to_string_lossy();
    if path_str.contains("..") {
        return Err((StatusCode::FORBIDDEN, "invalid path".to_string()));
    }

    // Security: resolve to canonical path, then verify it's within an allowed skill directory
    let canonical = path
        .canonicalize()
        .map_err(|_| (StatusCode::NOT_FOUND, "path not found".to_string()))?;
    let canonical_str = canonical.to_string_lossy();
    let home = dirs::home_dir().unwrap_or_default();
    let allowed_dirs = [
        home.join(".claude").join("skills"),
        home.join(".claude").join("commands"),
        home.join(".synapse").join("skills"),
    ];
    let is_skill_path = allowed_dirs.iter().any(|d| {
        d.exists()
            && d.canonicalize()
                .map(|cd| canonical_str.starts_with(&*cd.to_string_lossy()))
                .unwrap_or(false)
    });
    if !is_skill_path {
        return Err((
            StatusCode::FORBIDDEN,
            "only skill files can be read".to_string(),
        ));
    }

    let raw = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, format!("read skill: {}", e)))?;

    // Strip YAML frontmatter (---...---) only for .md files
    let is_md = path_str.ends_with(".md");
    let content = if is_md && raw.starts_with("---") {
        if let Some(end) = raw[3..].find("\n---") {
            raw[3 + end + 4..].trim_start_matches('\n').to_string()
        } else {
            raw
        }
    } else {
        raw
    };

    // Limit to 64KB to prevent huge responses
    let content = if content.len() > 65536 {
        format!("{}...\n\n[truncated at 64KB]", &content[..65536])
    } else {
        content
    };

    Ok(Json(SkillContentResponse { content }))
}

// ---------------------------------------------------------------------------
// GET /api/dashboard/skills/files — List files in a local skill directory
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct SkillFilesQuery {
    /// Path to the skill directory (parent of SKILL.md) or the SKILL.md file itself.
    path: String,
}

#[derive(Serialize)]
struct SkillFileEntry {
    name: String,
    size: u64,
}

#[derive(Serialize)]
struct SkillFilesListResponse {
    files: Vec<SkillFileEntry>,
}

async fn get_skill_files(
    Query(query): Query<SkillFilesQuery>,
) -> Result<Json<SkillFilesListResponse>, (StatusCode, String)> {
    let path = Path::new(&query.path);

    // Determine the skill directory: if path points to a .md file, use its parent
    let skill_dir = if path.is_file() {
        path.parent().unwrap_or(path)
    } else {
        path
    };

    // Prevent path traversal
    let dir_str = skill_dir.to_string_lossy();
    if dir_str.contains("..") {
        return Err((StatusCode::FORBIDDEN, "invalid path".to_string()));
    }

    // Security: resolve to canonical path, then verify it's within an allowed skill directory
    let canonical = skill_dir
        .canonicalize()
        .map_err(|_| (StatusCode::NOT_FOUND, "path not found".to_string()))?;
    let canonical_str = canonical.to_string_lossy();
    let home = dirs::home_dir().unwrap_or_default();
    let allowed_dirs = [
        home.join(".claude").join("skills"),
        home.join(".claude").join("commands"),
        home.join(".synapse").join("skills"),
    ];
    let is_skill_path = allowed_dirs.iter().any(|d| {
        d.exists()
            && d.canonicalize()
                .map(|cd| canonical_str.starts_with(&*cd.to_string_lossy()))
                .unwrap_or(false)
    });
    if !is_skill_path {
        return Err((
            StatusCode::FORBIDDEN,
            "only skill directories can be listed".to_string(),
        ));
    }

    if !skill_dir.exists() || !skill_dir.is_dir() {
        return Ok(Json(SkillFilesListResponse { files: vec![] }));
    }

    // Recursively collect files
    let mut files = Vec::new();
    collect_skill_files(skill_dir, skill_dir, &mut files, 0);
    files.sort_by(|a, b| a.name.cmp(&b.name));

    Ok(Json(SkillFilesListResponse { files }))
}

fn collect_skill_files(base: &Path, dir: &Path, out: &mut Vec<SkillFileEntry>, depth: usize) {
    // Limit recursion depth and total files to prevent DoS
    if depth > 5 || out.len() > 500 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if out.len() > 500 {
            return;
        }
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with('.') {
            continue; // skip hidden files
        }
        if path.is_dir() {
            collect_skill_files(base, &path, out, depth + 1);
        } else {
            let rel = path
                .strip_prefix(base)
                .map(|p| p.to_string_lossy().to_string())
                .unwrap_or_else(|_| name.to_string());
            let size = path.metadata().map(|m| m.len()).unwrap_or(0);
            out.push(SkillFileEntry { name: rel, size });
        }
    }
}

// ---------------------------------------------------------------------------
// 11. GET /api/dashboard/mcp
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
// 12. GET /api/dashboard/requests
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
// 13. GET /api/dashboard/logs?lines=100&level=error
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

    let mut latest: Option<(std::path::PathBuf, std::time::SystemTime)> = None;
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

    // Apply level filter if specified
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
// Phase 9: GET /api/dashboard/logs/export
// ---------------------------------------------------------------------------

async fn export_logs(
    State(_state): State<AppState>,
) -> Result<(StatusCode, [(String, String); 2], Vec<u8>), (StatusCode, String)> {
    let log_dir = Path::new("log");

    if !log_dir.exists() {
        return Err((StatusCode::NOT_FOUND, "no log directory".to_string()));
    }

    // Find latest log file
    let mut latest: Option<(std::path::PathBuf, std::time::SystemTime)> = None;
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
// Phase 4: GET /api/dashboard/agents
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

    // Default agent
    agents.push(AgentResponse {
        name: "default".to_string(),
        model: state.config.base.model.model.clone(),
        system_prompt: state.config.base.agent.system_prompt.clone(),
        channels: vec![],
        workspace: Some(state.config.workspace_dir().to_string_lossy().to_string()),
        is_default: true,
    });

    // Agent routes
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
// Phase 4: POST /api/dashboard/agents
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

    // Check for duplicate name
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
// Phase 4: PUT /api/dashboard/agents/{name}
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
// Phase 4: DELETE /api/dashboard/agents/{name}
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
    source: String, // "core", "filesystem", "agent", "memory", "session", "mcp"
}

#[derive(Serialize)]
struct ToolCatalogGroup {
    id: String,
    label: String,
    tools: Vec<ToolCatalogEntry>,
}

async fn get_tools_catalog(State(state): State<AppState>) -> Json<Vec<ToolCatalogGroup>> {
    let mut groups = Vec::new();

    // 1. Filesystem tools (always present)
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

    // 2. Core tools (built-in synapse tools)
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

    // Conditional tools based on features/config
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

    // 3. Agent tools (framework-level)
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

    // 4. Memory tools (if LTM enabled)
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

    // 5. Session tools (always present when gateway is running)
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

    // 6. MCP tools (from config)
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
// Phase 8: POST /api/dashboard/debug/invoke
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
            let sessions = state.sessions.list_sessions().await.map(|s| s.len()).unwrap_or(0);
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
        "version" => {
            Json(DebugInvokeResponse {
                ok: true,
                result: Some(serde_json::json!({
                    "version": env!("CARGO_PKG_VERSION"),
                    "name": env!("CARGO_PKG_NAME"),
                })),
                error: None,
            })
        }
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
            let list: Vec<_> = sessions.iter().map(|s| serde_json::json!({
                "id": s.id,
            })).collect();
            Json(DebugInvokeResponse {
                ok: true,
                result: Some(serde_json::json!(list)),
                error: None,
            })
        }
        "schedules" => {
            let schedules: Vec<_> = state.config.schedules.as_ref().map(|entries| {
                entries.iter().map(|s| serde_json::json!({
                    "name": s.name,
                    "prompt": s.prompt,
                    "cron": s.cron,
                    "interval_secs": s.interval_secs,
                    "enabled": s.enabled,
                })).collect()
            }).unwrap_or_default();
            Json(DebugInvokeResponse {
                ok: true,
                result: Some(serde_json::json!(schedules)),
                error: None,
            })
        }
        _ => Json(DebugInvokeResponse {
            ok: false,
            result: None,
            error: Some(format!("unknown method: {}. Available: health, cost_snapshot, stats, version, providers, models.list, sessions, schedules", body.method)),
        }),
    }
}

// ---------------------------------------------------------------------------
// Phase 8: GET /api/dashboard/debug/health
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct DebugHealthResponse {
    status: String,
    uptime_secs: u64,
    memory_rss_mb: Option<f64>,
    active_connections: usize,
    active_sessions: usize,
}

async fn debug_health(State(state): State<AppState>) -> Json<DebugHealthResponse> {
    let active = state.cancel_tokens.read().await.len();
    let sessions = state
        .sessions
        .list_sessions()
        .await
        .map(|s| s.len())
        .unwrap_or(0);

    // Try to get process memory on macOS/Linux
    let memory_rss_mb = {
        #[cfg(unix)]
        {
            std::fs::read_to_string("/proc/self/statm")
                .ok()
                .and_then(|s| s.split_whitespace().nth(1)?.parse::<u64>().ok())
                .map(|pages| (pages * 4096) as f64 / (1024.0 * 1024.0))
        }
        #[cfg(not(unix))]
        {
            None::<f64>
        }
    };

    Json(DebugHealthResponse {
        status: "ok".to_string(),
        uptime_secs: state.started_at.elapsed().as_secs(),
        memory_rss_mb,
        active_connections: active,
        active_sessions: sessions,
    })
}

// ---------------------------------------------------------------------------
// Phase 10: GET /api/dashboard/version
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
// Helpers
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct OkResponse {
    ok: bool,
}

fn config_file_path() -> String {
    if Path::new("synapse.toml").exists() {
        "synapse.toml".to_string()
    } else {
        "synapse.toml.example".to_string()
    }
}

async fn read_config_file() -> Result<(String, String), (StatusCode, String)> {
    let path = config_file_path();
    let content = tokio::fs::read_to_string(&path).await.map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("read config: {}", e),
        )
    })?;
    Ok((path, content))
}

fn count_bot_channels(config: &crate::config::SynapseConfig) -> usize {
    let checks: &[bool] = &[
        config.lark.is_some(),
        config.slack.is_some(),
        config.telegram.is_some(),
        config.discord.is_some(),
        config.dingtalk.is_some(),
        config.mattermost.is_some(),
        config.matrix.is_some(),
        config.whatsapp.is_some(),
        config.teams.is_some(),
        config.signal.is_some(),
        config.wechat.is_some(),
        config.imessage.is_some(),
        config.line.is_some(),
        config.googlechat.is_some(),
        config.irc.is_some(),
        config.webchat.is_some(),
        config.twitch.is_some(),
        config.nostr.is_some(),
        config.nextcloud.is_some(),
        config.synology.is_some(),
        config.tlon.is_some(),
        config.zalo.is_some(),
    ];
    checks.iter().filter(|&&v| v).count()
}

fn parse_system_time_string(s: &str) -> String {
    if let Some(sec_start) = s.find("tv_sec: ") {
        let rest = &s[sec_start + 8..];
        if let Some(end) = rest.find(|c: char| !c.is_ascii_digit()) {
            if let Ok(secs) = rest[..end].parse::<u64>() {
                return (secs * 1000).to_string();
            }
        }
    }
    s.to_string()
}

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

/// Parse full skill info from a SKILL.md or flat .md file for the dashboard API.
///
/// Returns: (description, user_invocable, eligible, emoji, homepage, version, missing_env, missing_bins, has_install)
#[allow(clippy::type_complexity)]
async fn parse_skill_full_info(
    path: &Path,
) -> (
    String,
    bool,
    bool,
    Option<String>,
    Option<String>,
    Option<String>,
    Vec<String>,
    Vec<String>,
    bool,
) {
    let content = match tokio::fs::read_to_string(path).await {
        Ok(c) => c,
        Err(_) => {
            return (
                String::new(),
                true,
                true,
                None,
                None,
                None,
                Vec::new(),
                Vec::new(),
                false,
            )
        }
    };

    let content = content.trim_start_matches('\u{feff}');
    let mut lines = content.lines();

    if lines.next().map(|l| l.trim()) != Some("---") {
        return (
            String::new(),
            true,
            true,
            None,
            None,
            None,
            Vec::new(),
            Vec::new(),
            false,
        );
    }

    let mut fm_lines = Vec::new();
    for line in lines {
        if line.trim() == "---" {
            break;
        }
        fm_lines.push(line);
    }

    let yaml_str = fm_lines.join("\n");
    let yaml: serde_json::Value = match serde_yml::from_str(&yaml_str) {
        Ok(v) => v,
        Err(_) => {
            return (
                String::new(),
                true,
                true,
                None,
                None,
                None,
                Vec::new(),
                Vec::new(),
                false,
            )
        }
    };
    let map = match yaml.as_object() {
        Some(m) => m,
        None => {
            return (
                String::new(),
                true,
                true,
                None,
                None,
                None,
                Vec::new(),
                Vec::new(),
                false,
            )
        }
    };

    let description = map
        .get("description")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    let user_invocable = map
        .get("user-invocable")
        .or_else(|| map.get("user_invocable"))
        .and_then(|v| v.as_bool())
        .unwrap_or(true);
    let version = map
        .get("version")
        .and_then(|v| v.as_str())
        .map(String::from);

    // Extract from metadata.openclaw
    let oc = map
        .get("metadata")
        .and_then(|m| m.get("openclaw").or_else(|| m.get("clawdbot")));
    let homepage = map
        .get("homepage")
        .and_then(|v| v.as_str())
        .map(String::from)
        .or_else(|| {
            oc.and_then(|o| o.get("homepage"))
                .and_then(|v| v.as_str())
                .map(String::from)
        });
    let emoji = map
        .get("emoji")
        .and_then(|v| v.as_str())
        .map(String::from)
        .or_else(|| {
            oc.and_then(|o| o.get("emoji"))
                .and_then(|v| v.as_str())
                .map(String::from)
        });
    let has_install = oc
        .and_then(|o| o.get("install"))
        .and_then(|i| i.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false);

    // Check requirements
    let mut missing_env = Vec::new();
    let required_env: Vec<String> = map
        .get("required-env")
        .or_else(|| map.get("required_env"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    for e in &required_env {
        if std::env::var(e).is_err() {
            missing_env.push(e.clone());
        }
    }

    let mut missing_bins = Vec::new();
    let required_bins: Vec<String> = map
        .get("required-bins")
        .or_else(|| map.get("required_bins"))
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default();
    for b in &required_bins {
        if which::which(b).is_err() {
            missing_bins.push(b.clone());
        }
    }

    let eligible = missing_env.is_empty() && missing_bins.is_empty();

    (
        description,
        user_invocable,
        eligible,
        emoji,
        homepage,
        version,
        missing_env,
        missing_bins,
        has_install,
    )
}

async fn parse_skill_frontmatter(path: &Path) -> (String, bool) {
    let content = match tokio::fs::read_to_string(path).await {
        Ok(c) => c,
        Err(_) => return (String::new(), false),
    };

    let mut description = String::new();
    let mut user_invocable = false;

    // Simple YAML frontmatter parser (between --- delimiters)
    if content.starts_with("---") {
        if let Some(end) = content[3..].find("---") {
            let frontmatter = &content[3..3 + end];
            for line in frontmatter.lines() {
                let line = line.trim();
                if let Some(val) = line.strip_prefix("description:") {
                    description = val.trim().trim_matches('"').to_string();
                }
                if let Some(val) = line.strip_prefix("user-invocable:") {
                    user_invocable = val.trim() == "true";
                }
            }
        }
    }

    (description, user_invocable)
}

// ---------------------------------------------------------------------------
// Workspace files CRUD
// ---------------------------------------------------------------------------

fn sanitize_workspace_filename(filename: &str) -> Result<(), (StatusCode, String)> {
    if filename.is_empty() || filename.len() > 64 {
        return Err((
            StatusCode::BAD_REQUEST,
            "filename must be 1-64 characters".to_string(),
        ));
    }
    if !filename.ends_with(".md") {
        return Err((
            StatusCode::BAD_REQUEST,
            "filename must end with .md".to_string(),
        ));
    }
    if filename.contains("..")
        || filename.contains('/')
        || filename.contains('\\')
        || filename.contains('\0')
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "invalid characters in filename".to_string(),
        ));
    }
    if !filename
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-')
    {
        return Err((
            StatusCode::BAD_REQUEST,
            "filename may only contain [a-zA-Z0-9._-]".to_string(),
        ));
    }
    Ok(())
}

fn workspace_dir(config: &crate::config::SynapseConfig, agent: Option<&str>) -> PathBuf {
    config.workspace_dir_for_agent(agent)
}

/// Query parameter for per-agent workspace resolution.
#[derive(Deserialize)]
struct WorkspaceQuery {
    /// Optional agent name. If set, resolves to the agent-specific workspace directory.
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

    // Also scan for custom .md files in cwd that aren't templates
    if let Ok(mut dir) = tokio::fs::read_dir(&cwd).await {
        while let Ok(Some(entry)) = dir.next_entry().await {
            let name = entry.file_name().to_string_lossy().to_string();
            if !name.ends_with(".md") {
                continue;
            }
            // Skip if already a template
            if WORKSPACE_TEMPLATES.iter().any(|t| t.filename == name) {
                continue;
            }
            // Skip common files that aren't workspace context
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
            format!("file '{}' not found — use POST to create", filename),
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
            format!("file '{}' already exists — use PUT to update", filename),
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
// ClawHub integration
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct HubSearchQuery {
    q: String,
    #[serde(default = "default_hub_limit")]
    limit: usize,
}

#[derive(Deserialize)]
struct HubListQuery {
    #[serde(default = "default_hub_limit")]
    limit: usize,
    sort: Option<String>,
    cursor: Option<String>,
}

#[derive(Deserialize)]
struct HubInstallBody {
    slug: String,
    version: Option<String>,
}

fn default_hub_limit() -> usize {
    20
}

async fn store_search(
    State(state): State<AppState>,
    Query(query): Query<HubSearchQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let hub = crate::hub::ClawHubClient::from_config(&state.config);
    let results = hub
        .search(&query.q, query.limit)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("store search: {}", e)))?;
    Ok(Json(
        serde_json::json!({ "results": results, "source": "clawhub" }),
    ))
}

async fn store_list(
    State(state): State<AppState>,
    Query(query): Query<HubListQuery>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let hub = crate::hub::ClawHubClient::from_config(&state.config);
    let items = hub
        .list(query.limit, query.sort.as_deref(), query.cursor.as_deref())
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("store list: {}", e)))?;
    Ok(Json(
        serde_json::json!({ "items": items, "source": "clawhub" }),
    ))
}

async fn store_detail(
    State(state): State<AppState>,
    extract::Path(slug): extract::Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let hub = crate::hub::ClawHubClient::from_config(&state.config);
    let detail = hub
        .detail(&slug)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("store detail: {}", e)))?;
    Ok(Json(detail))
}

async fn store_skill_files(
    State(state): State<AppState>,
    extract::Path(slug): extract::Path<String>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let hub = crate::hub::ClawHubClient::from_config(&state.config);
    let files_resp = hub
        .skill_files(&slug)
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("store files: {}", e)))?;
    Ok(Json(serde_json::json!({
        "files": files_resp.files,
        "skillMd": files_resp.skill_md,
    })))
}

async fn store_skill_file_content(
    State(state): State<AppState>,
    extract::Path((slug, path)): extract::Path<(String, String)>,
) -> Result<Json<serde_json::Value>, (StatusCode, String)> {
    let hub = crate::hub::ClawHubClient::from_config(&state.config);
    let content = hub.skill_file_content(&slug, &path).await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            format!("store file content: {}", e),
        )
    })?;
    Ok(Json(serde_json::json!({
        "content": content,
    })))
}

async fn store_install(
    State(state): State<AppState>,
    Json(body): Json<HubInstallBody>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    let hub = crate::hub::ClawHubClient::from_config(&state.config);
    crate::hub::install::install_from_hub(&hub, &body.slug, body.version.as_deref(), false)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("install: {}", e)))?;
    Ok(Json(OkResponse { ok: true }))
}

async fn store_status(State(state): State<AppState>) -> Json<serde_json::Value> {
    let hub = crate::hub::ClawHubClient::from_config(&state.config);
    let configured = hub.is_configured();
    let lock = crate::hub::install::read_lock_file();
    let installed_count = lock.skills.len();
    let installed: Vec<String> = lock.skills.keys().cloned().collect();
    Json(serde_json::json!({
        "configured": configured,
        "installedCount": installed_count,
        "installed": installed,
        "source": "clawhub",
    }))
}
