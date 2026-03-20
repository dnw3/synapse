use axum::extract::State;
use axum::response::Json;
use axum::routing::post;
use axum::Router;
use serde::{Deserialize, Serialize};

use crate::gateway::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new().route("/dashboard/debug/invoke", post(debug_invoke))
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
