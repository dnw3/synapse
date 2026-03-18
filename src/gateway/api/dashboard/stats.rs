use std::collections::HashMap;

use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::get;
use axum::Router;
use serde::{Deserialize, Serialize};
use synaptic::core::MemoryStore;

use super::count_bot_channels;
use crate::gateway::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/dashboard/stats", get(get_stats))
        .route("/dashboard/usage", get(get_usage))
        .route("/dashboard/usage/timeseries", get(get_usage_timeseries))
        .route("/dashboard/usage/sessions", get(get_usage_sessions))
        .route("/dashboard/providers", get(get_providers))
        .route("/dashboard/health", get(get_health))
        .route("/dashboard/debug/health", get(debug_health))
}

// ---------------------------------------------------------------------------
// GET /api/dashboard/stats
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
        total_messages += memory
            .load(&s.session_id)
            .await
            .map(|m| m.len())
            .unwrap_or(0);
    }

    let usage_snap = state.usage_tracker.snapshot().await;
    let active_ws = state.cancel_tokens.read().await.len();

    Ok(Json(StatsResponse {
        session_count: sessions.len(),
        total_messages,
        total_input_tokens: usage_snap.totals.input_tokens,
        total_output_tokens: usage_snap.totals.output_tokens,
        total_cost_usd: usage_snap.totals.total_cost,
        uptime_secs: state.started_at.elapsed().as_secs(),
        active_ws_sessions: active_ws,
    }))
}

// ---------------------------------------------------------------------------
// GET /api/dashboard/usage
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
    let snap = state.usage_tracker.snapshot().await;

    let per_model: Vec<ModelUsageEntry> = snap
        .by_model
        .into_iter()
        .map(|d| ModelUsageEntry {
            model: d.key,
            input_tokens: d.input_tokens,
            output_tokens: d.output_tokens,
            requests: d.count,
            cost_usd: d.cost,
        })
        .collect();

    Json(UsageResponse {
        total_input_tokens: snap.totals.input_tokens,
        total_output_tokens: snap.totals.output_tokens,
        total_requests: snap.totals.request_count,
        total_cost_usd: snap.totals.total_cost,
        per_model,
    })
}

// ---------------------------------------------------------------------------
// GET /api/dashboard/usage/timeseries
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
    let snap = state.usage_tracker.snapshot().await;

    let entries: Vec<TimeseriesEntry> = snap
        .daily
        .into_iter()
        .map(|d| TimeseriesEntry {
            timestamp: format!("{}T00:00:00Z", d.date),
            input_tokens: d.input_tokens,
            output_tokens: d.output_tokens,
            cost: d.cost,
            count: d.count,
        })
        .collect();

    Json(entries)
}

// ---------------------------------------------------------------------------
// GET /api/dashboard/usage/sessions
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
        let msg_count = memory
            .load(&s.session_id)
            .await
            .map(|msgs| msgs.len() as u64)
            .unwrap_or(0);

        let input_tokens = (s.total_tokens as f64 * 0.6) as u64;
        let output_tokens = s.total_tokens.saturating_sub(input_tokens);

        result.push(UsageSessionEntry {
            session_id: s.session_id,
            input_tokens,
            output_tokens,
            cost: 0.0,
            request_count: msg_count / 2,
        });
    }

    Ok(Json(result))
}

// ---------------------------------------------------------------------------
// GET /api/dashboard/providers
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
// GET /api/dashboard/health
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
        memory_entries += memory
            .load(&s.session_id)
            .await
            .map(|m| m.len())
            .unwrap_or(0);
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
// GET /api/dashboard/debug/health
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
