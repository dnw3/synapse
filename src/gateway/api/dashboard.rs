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
        .route(
            "/dashboard/schedules/{name}/runs",
            get(get_schedule_runs),
        )
        // Phase 3: Config advanced
        .route("/dashboard/config/validate", post(validate_config))
        .route("/dashboard/config/reload", post(reload_config))
        // Phase 4: Agents
        .route("/dashboard/agents", get(get_agents))
        .route("/dashboard/agents", post(create_agent))
        .route("/dashboard/agents/{name}", put(update_agent))
        .route("/dashboard/agents/{name}", delete(delete_agent))
        // Phase 5: Sessions CRUD
        .route("/dashboard/sessions/{id}", delete(delete_session))
        .route("/dashboard/sessions/{id}", patch(patch_session))
        .route(
            "/dashboard/sessions/{id}/compact",
            post(compact_session),
        )
        // Phase 6: Channels toggle + config
        .route(
            "/dashboard/channels/{name}/toggle",
            post(toggle_channel),
        )
        .route(
            "/dashboard/channels/{name}/config",
            put(put_channel_config),
        )
        // Phase 7: Skills toggle
        .route(
            "/dashboard/skills/{name}/toggle",
            post(toggle_skill),
        )
        .route("/dashboard/skills/content", get(get_skill_content))
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
        .route("/dashboard/workspace/{filename}", post(create_workspace_file))
        .route("/dashboard/workspace/{filename}", delete(delete_workspace_file))
        .route("/dashboard/workspace/{filename}/reset", post(reset_workspace_file))
        .route("/dashboard/identity", get(get_identity))
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
            cost: 0.0, // per-session cost tracking not yet available
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
            let provider = m
                .provider
                .clone()
                .unwrap_or_else(|| "default".to_string());
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
                base_url: state
                    .config
                    .base
                    .model
                    .base_url
                    .clone()
                    .unwrap_or_default(),
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
        let count = memory.load(&s.id).await.map(|m| m.len()).unwrap_or(0);
        let ovr = overrides.get(&s.id);
        result.push(SessionResponse {
            id: s.id,
            created_at: parse_system_time_string(&s.created_at),
            message_count: count,
            token_count: s.token_count,
            compaction_count: s.compaction_count,
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

    save_overrides(&overrides)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

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
    let mut doc: toml::Value = toml::from_str(&content)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("parse TOML: {}", e)))?;

    let new_entry = build_schedule_toml(&body.name, &body.prompt, &body.cron, &body.interval_secs, body.enabled, &body.description);

    let schedules = doc
        .as_table_mut()
        .unwrap()
        .entry("schedule")
        .or_insert_with(|| toml::Value::Array(Vec::new()));
    if let toml::Value::Array(arr) = schedules {
        arr.push(new_entry);
    }

    let new_content = toml::to_string_pretty(&doc)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("serialize: {}", e)))?;
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
    let mut doc: toml::Value = toml::from_str(&content)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("parse TOML: {}", e)))?;

    if let Some(toml::Value::Array(arr)) = doc.get_mut("schedule") {
        if let Some(pos) = arr.iter().position(|s| {
            s.get("name").and_then(|n| n.as_str()) == Some(&name)
        }) {
            arr[pos] = build_schedule_toml(&body.name, &body.prompt, &body.cron, &body.interval_secs, body.enabled, &body.description);
        } else {
            return Err((StatusCode::NOT_FOUND, format!("schedule '{}' not found", name)));
        }
    } else {
        return Err((StatusCode::NOT_FOUND, "no schedules configured".to_string()));
    }

    let new_content = toml::to_string_pretty(&doc)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("serialize: {}", e)))?;
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
    let mut doc: toml::Value = toml::from_str(&content)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("parse TOML: {}", e)))?;

    if let Some(toml::Value::Array(arr)) = doc.get_mut("schedule") {
        let before = arr.len();
        arr.retain(|s| s.get("name").and_then(|n| n.as_str()) != Some(&name));
        if arr.len() == before {
            return Err((StatusCode::NOT_FOUND, format!("schedule '{}' not found", name)));
        }
    }

    let new_content = toml::to_string_pretty(&doc)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("serialize: {}", e)))?;
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
    let mut doc: toml::Value = toml::from_str(&content)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("parse TOML: {}", e)))?;

    let mut new_enabled = true;
    if let Some(toml::Value::Array(arr)) = doc.get_mut("schedule") {
        if let Some(entry) = arr.iter_mut().find(|s| {
            s.get("name").and_then(|n| n.as_str()) == Some(&name)
        }) {
            let current = entry
                .get("enabled")
                .and_then(|v| v.as_bool())
                .unwrap_or(true);
            new_enabled = !current;
            if let Some(tbl) = entry.as_table_mut() {
                tbl.insert("enabled".to_string(), toml::Value::Boolean(new_enabled));
            }
        } else {
            return Err((StatusCode::NOT_FOUND, format!("schedule '{}' not found", name)));
        }
    }

    let new_content = toml::to_string_pretty(&doc)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("serialize: {}", e)))?;
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
    tokio::fs::write(&path, &body.content)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("write failed: {}", e)))?;

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
    match toml::from_str::<toml::Value>(&body.content) {
        Ok(_) => Json(ValidateConfigResponse {
            valid: true,
            errors: vec![],
        }),
        Err(e) => Json(ValidateConfigResponse {
            valid: false,
            errors: vec![e.to_string()],
        }),
    }
}

// ---------------------------------------------------------------------------
// Phase 3: POST /api/dashboard/config/reload
// ---------------------------------------------------------------------------

async fn reload_config(
    State(_state): State<AppState>,
) -> Json<OkResponse> {
    // Config reload would require AppState to hold a mutable config reference
    // Placeholder for now
    Json(OkResponse { ok: true })
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
    let mut doc: toml::Value = toml::from_str(&content)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("parse TOML: {}", e)))?;

    // Determine current enabled state: prefer channel_overrides from TOML,
    // then check if channel section exists in TOML (not in-memory config).
    let known_channels = [
        "lark", "slack", "telegram", "discord", "dingtalk", "mattermost",
        "matrix", "whatsapp", "teams", "signal", "wechat", "imessage",
        "line", "googlechat", "irc", "webchat", "twitch", "nostr",
        "nextcloud", "synology", "tlon", "zalo",
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

    let new_content = toml::to_string_pretty(&doc)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("serialize: {}", e)))?;
    tokio::fs::write(&path, &new_content)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("write: {}", e)))?;

    let enabled = new_enabled;
    tracing::info!(channel = %name, enabled, "channel toggled");

    Ok(Json(ToggleResponse { enabled: new_enabled }))
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
    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("read failed: {}", e)))?;

    let mut doc: toml::Value = toml::from_str(&content)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("parse failed: {}", e)))?;

    // Ensure root is a table
    let root = doc
        .as_table_mut()
        .ok_or_else(|| (StatusCode::INTERNAL_SERVER_ERROR, "config root is not a table".to_string()))?;

    // Get or create the channel section
    if !root.contains_key(&name) {
        root.insert(name.clone(), toml::Value::Table(Default::default()));
    }
    let section = root
        .get_mut(&name)
        .and_then(|v| v.as_table_mut())
        .ok_or_else(|| (StatusCode::INTERNAL_SERVER_ERROR, "channel section is not a table".to_string()))?;

    // Update fields
    for (key, value) in body {
        if value.is_empty() {
            section.remove(&key);
        } else {
            section.insert(key, toml::Value::String(value));
        }
    }

    let new_content = toml::to_string_pretty(&doc)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("serialize failed: {}", e)))?;
    tokio::fs::write(&path, &new_content)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("write failed: {}", e)))?;

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
}

async fn get_skills(
    State(state): State<AppState>,
) -> Result<Json<Vec<SkillResponse>>, (StatusCode, String)> {
    let mut skills = Vec::new();

    let dirs: Vec<(&str, String)> = vec![
        ("project", ".claude/skills".to_string()),
        (
            "personal",
            dirs::home_dir()
                .map(|h| h.join(".claude/skills").to_string_lossy().to_string())
                .unwrap_or_default(),
        ),
    ];

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

                    // Parse frontmatter for description and user_invocable
                    let (description, user_invocable) = parse_skill_frontmatter(&path).await;

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
    let mut doc: toml::Value = toml::from_str(&content)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("parse TOML: {}", e)))?;

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

    let new_content = toml::to_string_pretty(&doc)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("serialize: {}", e)))?;
    tokio::fs::write(&path, &new_content)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("write: {}", e)))?;

    Ok(Json(ToggleResponse { enabled: new_enabled }))
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

    // Security: only allow reading .md files from known skill directories
    let path_str = path.to_string_lossy();
    let is_skill_path = path_str.contains("/.claude/skills/")
        || path_str.contains("/.claude/commands/");
    if !is_skill_path || !path_str.ends_with(".md") {
        return Err((StatusCode::FORBIDDEN, "only skill .md files can be read".to_string()));
    }

    let content = tokio::fs::read_to_string(path)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, format!("read skill: {}", e)))?;

    // Limit to first 5KB to prevent huge responses
    let content = if content.len() > 5120 {
        format!("{}...\n\n[truncated at 5KB]", &content[..5120])
    } else {
        content
    };

    Ok(Json(SkillContentResponse { content }))
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
            avg_duration_secs: if *count > 0 {
                sum / *count as f64
            } else {
                0.0
            },
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

    let content = tokio::fs::read_to_string(&log_file)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("read log: {}", e)))?;

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
}

async fn get_agents(State(state): State<AppState>) -> Json<Vec<AgentResponse>> {
    let mut agents = Vec::new();

    // Default agent
    agents.push(AgentResponse {
        name: "default".to_string(),
        model: state.config.base.model.model.clone(),
        system_prompt: state.config.base.agent.system_prompt.clone(),
        channels: vec![],
        is_default: true,
    });

    // Agent routes
    if let Some(routes) = &state.config.agent_routes {
        for route in routes {
            agents.push(AgentResponse {
                name: route.name.clone(),
                model: route.model.clone().unwrap_or_else(|| state.config.base.model.model.clone()),
                system_prompt: route.system_prompt.clone(),
                channels: route.channels.clone(),
                is_default: false,
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
}

async fn create_agent(
    State(state): State<AppState>,
    Json(body): Json<CreateAgentRequest>,
) -> Result<Json<AgentResponse>, (StatusCode, String)> {
    if body.name == "default" {
        return Err((StatusCode::BAD_REQUEST, "cannot create agent named 'default'".to_string()));
    }

    let (path, content) = read_config_file().await?;
    let mut doc: toml::Value = toml::from_str(&content)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("parse TOML: {}", e)))?;

    // Check for duplicate name
    if let Some(toml::Value::Array(arr)) = doc.get("agent_routes") {
        if arr.iter().any(|r| r.get("name").and_then(|n| n.as_str()) == Some(&body.name)) {
            return Err((StatusCode::CONFLICT, format!("agent '{}' already exists", body.name)));
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

    let new_content = toml::to_string_pretty(&doc)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("serialize: {}", e)))?;
    tokio::fs::write(&path, &new_content)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("write: {}", e)))?;

    Ok(Json(AgentResponse {
        name: body.name,
        model: body.model.unwrap_or_else(|| state.config.base.model.model.clone()),
        system_prompt: body.system_prompt,
        channels: body.channels,
        is_default: false,
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
        return Err((StatusCode::BAD_REQUEST, "cannot modify the default agent via this endpoint".to_string()));
    }

    let (path, content) = read_config_file().await?;
    let mut doc: toml::Value = toml::from_str(&content)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("parse TOML: {}", e)))?;

    if let Some(toml::Value::Array(arr)) = doc.get_mut("agent_routes") {
        if let Some(pos) = arr.iter().position(|r| {
            r.get("name").and_then(|n| n.as_str()) == Some(&name)
        }) {
            arr[pos] = build_agent_route_toml(&body);
        } else {
            return Err((StatusCode::NOT_FOUND, format!("agent '{}' not found", name)));
        }
    } else {
        return Err((StatusCode::NOT_FOUND, "no agent_routes configured".to_string()));
    }

    let new_content = toml::to_string_pretty(&doc)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("serialize: {}", e)))?;
    tokio::fs::write(&path, &new_content)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("write: {}", e)))?;

    tracing::info!(agent = %name, "agent updated");

    Ok(Json(AgentResponse {
        name: body.name,
        model: body.model.unwrap_or_else(|| state.config.base.model.model.clone()),
        system_prompt: body.system_prompt,
        channels: body.channels,
        is_default: false,
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
        return Err((StatusCode::BAD_REQUEST, "cannot delete the default agent".to_string()));
    }

    let (path, content) = read_config_file().await?;
    let mut doc: toml::Value = toml::from_str(&content)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("parse TOML: {}", e)))?;

    if let Some(toml::Value::Array(arr)) = doc.get_mut("agent_routes") {
        let before = arr.len();
        arr.retain(|r| r.get("name").and_then(|n| n.as_str()) != Some(&name));
        if arr.len() == before {
            return Err((StatusCode::NOT_FOUND, format!("agent '{}' not found", name)));
        }
    } else {
        return Err((StatusCode::NOT_FOUND, format!("agent '{}' not found", name)));
    }

    let new_content = toml::to_string_pretty(&doc)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("serialize: {}", e)))?;
    tokio::fs::write(&path, &new_content)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("write: {}", e)))?;

    Ok(Json(OkResponse { ok: true }))
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
    if Path::new("synaptic.toml").exists() {
        "synaptic.toml".to_string()
    } else {
        "synaptic.toml.example".to_string()
    }
}

async fn read_config_file() -> Result<(String, String), (StatusCode, String)> {
    let path = config_file_path();
    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("read config: {}", e)))?;
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
    tbl.insert("prompt".to_string(), toml::Value::String(prompt.to_string()));
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
    tbl.insert(
        "name".to_string(),
        toml::Value::String(body.name.clone()),
    );
    if let Some(ref model) = body.model {
        tbl.insert("model".to_string(), toml::Value::String(model.clone()));
    }
    if let Some(ref sp) = body.system_prompt {
        tbl.insert(
            "system_prompt".to_string(),
            toml::Value::String(sp.clone()),
        );
    }
    if let Some(ref desc) = body.description {
        tbl.insert(
            "description".to_string(),
            toml::Value::String(desc.clone()),
        );
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
    toml::Value::Table(tbl)
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
        return Err((StatusCode::BAD_REQUEST, "filename must be 1-64 characters".to_string()));
    }
    if !filename.ends_with(".md") {
        return Err((StatusCode::BAD_REQUEST, "filename must end with .md".to_string()));
    }
    if filename.contains("..") || filename.contains('/') || filename.contains('\\') || filename.contains('\0') {
        return Err((StatusCode::BAD_REQUEST, "invalid characters in filename".to_string()));
    }
    if !filename.chars().all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '_' || c == '-') {
        return Err((StatusCode::BAD_REQUEST, "filename may only contain [a-zA-Z0-9._-]".to_string()));
    }
    Ok(())
}

fn workspace_dir() -> PathBuf {
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
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
    State(_state): State<AppState>,
) -> Json<Vec<WorkspaceFileEntry>> {
    use crate::agent::templates::WORKSPACE_TEMPLATES;

    let cwd = workspace_dir();
    let mut entries = Vec::new();

    for tmpl in WORKSPACE_TEMPLATES {
        let path = cwd.join(tmpl.filename);
        let (exists, size_bytes, modified, preview) = if path.exists() {
            let meta = tokio::fs::metadata(&path).await.ok();
            let size = meta.as_ref().map(|m| m.len());
            let mod_time = meta
                .as_ref()
                .and_then(|m| m.modified().ok())
                .map(|t| {
                    let dur = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
                    chrono::DateTime::from_timestamp(dur.as_secs() as i64, dur.subsec_nanos())
                        .map(|dt| dt.to_rfc3339())
                        .unwrap_or_default()
                });
            let preview = tokio::fs::read_to_string(&path)
                .await
                .ok()
                .map(|c| {
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
            let mod_time = meta
                .as_ref()
                .and_then(|m| m.modified().ok())
                .map(|t| {
                    let dur = t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default();
                    chrono::DateTime::from_timestamp(dur.as_secs() as i64, dur.subsec_nanos())
                        .map(|dt| dt.to_rfc3339())
                        .unwrap_or_default()
                });
            let preview = tokio::fs::read_to_string(&path)
                .await
                .ok()
                .map(|c| {
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
    extract::Path(filename): extract::Path<String>,
) -> Result<Json<WorkspaceFileContent>, (StatusCode, String)> {
    sanitize_workspace_filename(&filename)?;
    let path = workspace_dir().join(&filename);
    let content = tokio::fs::read_to_string(&path)
        .await
        .map_err(|_| (StatusCode::NOT_FOUND, format!("file '{}' not found", filename)))?;
    let is_template = crate::agent::templates::find_template(&filename).is_some();
    Ok(Json(WorkspaceFileContent { filename, content, is_template }))
}

#[derive(Deserialize)]
struct WorkspaceFileBody {
    content: String,
}

async fn put_workspace_file(
    extract::Path(filename): extract::Path<String>,
    Json(body): Json<WorkspaceFileBody>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    sanitize_workspace_filename(&filename)?;
    let path = workspace_dir().join(&filename);
    if !path.exists() {
        return Err((StatusCode::NOT_FOUND, format!("file '{}' not found — use POST to create", filename)));
    }
    tokio::fs::write(&path, &body.content)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("write: {}", e)))?;

    tracing::info!(file = %filename, "workspace file saved");

    Ok(Json(OkResponse { ok: true }))
}

async fn create_workspace_file(
    extract::Path(filename): extract::Path<String>,
    Json(body): Json<WorkspaceFileBody>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    sanitize_workspace_filename(&filename)?;
    let path = workspace_dir().join(&filename);
    if path.exists() {
        return Err((StatusCode::CONFLICT, format!("file '{}' already exists — use PUT to update", filename)));
    }
    tokio::fs::write(&path, &body.content)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("write: {}", e)))?;
    Ok(Json(OkResponse { ok: true }))
}

async fn delete_workspace_file(
    extract::Path(filename): extract::Path<String>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    sanitize_workspace_filename(&filename)?;
    let path = workspace_dir().join(&filename);
    if !path.exists() {
        return Err((StatusCode::NOT_FOUND, format!("file '{}' not found", filename)));
    }
    tokio::fs::remove_file(&path)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("delete: {}", e)))?;
    Ok(Json(OkResponse { ok: true }))
}

async fn reset_workspace_file(
    extract::Path(filename): extract::Path<String>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    sanitize_workspace_filename(&filename)?;
    let default = crate::agent::workspace::default_content_for(&filename)
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("no default template for '{}'", filename)))?;
    let path = workspace_dir().join(&filename);
    tokio::fs::write(&path, default)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("write: {}", e)))?;
    Ok(Json(OkResponse { ok: true }))
}

async fn get_identity() -> Json<crate::agent::workspace::IdentityInfo> {
    let path = workspace_dir().join("IDENTITY.md");
    let info = match tokio::fs::read_to_string(&path).await {
        Ok(content) => crate::agent::workspace::parse_identity(&content),
        Err(_) => crate::agent::workspace::IdentityInfo::default(),
    };
    Json(info)
}
