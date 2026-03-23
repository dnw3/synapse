use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::routing::get;
use axum::{Json, Router};
use serde::Serialize;

use crate::gateway::state::AppState;
use crate::gateway::trace_aggregator::{TraceAggregator, TraceListParams};

/// GET /api/traces — list traces with optional filters.
pub async fn list_traces(
    State(state): State<AppState>,
    Query(params): Query<TraceListParams>,
) -> Json<crate::gateway::trace_aggregator::TraceListResponse> {
    let aggregator = TraceAggregator::new(std::sync::Arc::new(state.infra.log_buffer.clone()));
    let response = aggregator.list(&params).await;
    Json(response)
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

/// GET /api/traces/:request_id — get a single trace with full span details.
pub async fn get_trace(
    State(state): State<AppState>,
    Path(request_id): Path<String>,
) -> impl IntoResponse {
    let aggregator = TraceAggregator::new(std::sync::Arc::new(state.infra.log_buffer.clone()));
    match aggregator.detail(&request_id).await {
        Some(trace) => (StatusCode::OK, Json(serde_json::to_value(trace).unwrap())).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(
                serde_json::to_value(ErrorResponse {
                    error: format!("Trace not found: {}", request_id),
                })
                .unwrap(),
            ),
        )
            .into_response(),
    }
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/traces", get(list_traces))
        .route("/traces/{request_id}", get(get_trace))
}
