use axum::extract::{Query, State};
use axum::routing::get;
use axum::{Json, Router};
use serde::{Deserialize, Serialize};

use crate::gateway::state::AppState;

#[derive(Deserialize)]
pub struct LogsQuery {
    /// Max entries to return. Default: 100
    pub limit: Option<usize>,
    /// Filter by level (ERROR, WARN, INFO, DEBUG, TRACE)
    pub level: Option<String>,
    /// Filter by request_id
    pub request_id: Option<String>,
    /// Filter entries from this ISO 8601 timestamp (inclusive)
    pub from: Option<String>,
    /// Filter entries up to this ISO 8601 timestamp (inclusive)
    pub to: Option<String>,
    /// Full-text keyword search across message, target, and fields (case-insensitive)
    pub keyword: Option<String>,
}

#[derive(Serialize)]
pub struct LogsResponse {
    pub entries: Vec<crate::logging::LogEntry>,
    pub total: usize,
}

/// GET /api/logs — query recent logs from in-memory buffer
pub async fn query_logs(
    State(state): State<AppState>,
    Query(params): Query<LogsQuery>,
) -> Json<LogsResponse> {
    let limit = params.limit.unwrap_or(100).min(1000);
    let entries = state
        .log_buffer
        .query(
            limit,
            params.level.as_deref(),
            params.request_id.as_deref(),
            params.from.as_deref(),
            params.to.as_deref(),
            params.keyword.as_deref(),
        )
        .await;
    let total = entries.len();
    Json(LogsResponse { entries, total })
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/logs", get(query_logs))
}
