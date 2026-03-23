//! Prometheus-compatible /metrics endpoint.
//!
//! Exposes the 6 core metrics from the plan:
//! 1. synapse_requests_total{method, path, status}        — counter
//! 2. synapse_request_duration_seconds{method, path}      — histogram (sum + count)
//! 3. synapse_active_sessions                             — gauge
//! 4. synapse_tokens_used_total{model, direction}         — counter
//! 5. synapse_llm_request_duration_seconds{model}         — histogram (sum + count)
//! 6. synapse_memory_entries                              — gauge

use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use synaptic::core::MemoryStore;

use super::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new().route("/metrics", get(metrics_handler))
}

async fn metrics_handler(
    axum::extract::State(state): axum::extract::State<AppState>,
) -> impl IntoResponse {
    let mut out = String::with_capacity(4096);

    // --- Server info + uptime (bonus, kept from before) ---
    let uptime = state.core.started_at.elapsed().as_secs();
    out.push_str("# HELP synapse_uptime_seconds Server uptime in seconds\n");
    out.push_str("# TYPE synapse_uptime_seconds gauge\n");
    out.push_str(&format!("synapse_uptime_seconds {}\n\n", uptime));

    // -------------------------------------------------------
    // 1. synapse_requests_total{method, path, status} counter
    // -------------------------------------------------------
    out.push_str("# HELP synapse_requests_total Total HTTP requests\n");
    out.push_str("# TYPE synapse_requests_total counter\n");
    {
        let reqs = state.infra.request_metrics.requests.read().await;
        let mut keys: Vec<_> = reqs.keys().collect();
        keys.sort();
        for (method, path, status) in keys {
            let count = reqs[&(method.clone(), path.clone(), *status)];
            out.push_str(&format!(
                "synapse_requests_total{{method=\"{}\",path=\"{}\",status=\"{}\"}} {}\n",
                method, path, status, count
            ));
        }
    }
    out.push('\n');

    // -------------------------------------------------------
    // 2. synapse_request_duration_seconds{method, path} histogram (sum + count)
    // -------------------------------------------------------
    out.push_str("# HELP synapse_request_duration_seconds HTTP request duration in seconds\n");
    out.push_str("# TYPE synapse_request_duration_seconds histogram\n");
    {
        let durs = state.infra.request_metrics.durations.read().await;
        let mut keys: Vec<_> = durs.keys().collect();
        keys.sort();
        for (method, path) in keys {
            let (count, sum) = durs[&(method.clone(), path.clone())];
            out.push_str(&format!(
                "synapse_request_duration_seconds_count{{method=\"{}\",path=\"{}\"}} {}\n",
                method, path, count
            ));
            out.push_str(&format!(
                "synapse_request_duration_seconds_sum{{method=\"{}\",path=\"{}\"}} {:.6}\n",
                method, path, sum
            ));
        }
    }
    out.push('\n');

    // -------------------------------------------------------
    // 3. synapse_active_sessions gauge
    // -------------------------------------------------------
    out.push_str("# HELP synapse_active_sessions Number of active WebSocket sessions\n");
    out.push_str("# TYPE synapse_active_sessions gauge\n");
    let active = state.session.cancel_tokens.read().await.len();
    out.push_str(&format!("synapse_active_sessions {}\n\n", active));

    // -------------------------------------------------------
    // 4. synapse_tokens_used_total{model, direction} counter
    // -------------------------------------------------------
    out.push_str("# HELP synapse_tokens_used_total Total tokens used\n");
    out.push_str("# TYPE synapse_tokens_used_total counter\n");
    {
        let snapshot = state.agent.cost_tracker.snapshot().await;
        if snapshot.per_model.is_empty() {
            // Emit aggregate totals without model label
            out.push_str(&format!(
                "synapse_tokens_used_total{{direction=\"input\"}} {}\n",
                snapshot.total_input_tokens
            ));
            out.push_str(&format!(
                "synapse_tokens_used_total{{direction=\"output\"}} {}\n",
                snapshot.total_output_tokens
            ));
        } else {
            let mut models: Vec<_> = snapshot.per_model.keys().collect();
            models.sort();
            for model in models {
                let usage = &snapshot.per_model[model];
                out.push_str(&format!(
                    "synapse_tokens_used_total{{model=\"{}\",direction=\"input\"}} {}\n",
                    model, usage.input_tokens
                ));
                out.push_str(&format!(
                    "synapse_tokens_used_total{{model=\"{}\",direction=\"output\"}} {}\n",
                    model, usage.output_tokens
                ));
            }
        }
    }
    out.push('\n');

    // -------------------------------------------------------
    // 5. synapse_llm_request_duration_seconds{model} histogram (sum + count)
    // -------------------------------------------------------
    out.push_str("# HELP synapse_llm_request_duration_seconds LLM request duration in seconds\n");
    out.push_str("# TYPE synapse_llm_request_duration_seconds histogram\n");
    {
        let llm_durs = state.infra.request_metrics.llm_durations.read().await;
        let mut models: Vec<_> = llm_durs.keys().collect();
        models.sort();
        for model in models {
            let (count, sum) = llm_durs[model];
            out.push_str(&format!(
                "synapse_llm_request_duration_seconds_count{{model=\"{}\"}} {}\n",
                model, count
            ));
            out.push_str(&format!(
                "synapse_llm_request_duration_seconds_sum{{model=\"{}\"}} {:.6}\n",
                model, sum
            ));
        }
    }
    out.push('\n');

    // -------------------------------------------------------
    // 6. synapse_memory_entries gauge
    // -------------------------------------------------------
    out.push_str("# HELP synapse_memory_entries Total message entries across all sessions\n");
    out.push_str("# TYPE synapse_memory_entries gauge\n");
    let memory_entries = {
        let memory = state.session.sessions.memory();
        let sessions = state.session.sessions.list_sessions().await.unwrap_or_default();
        let mut total = 0usize;
        for s in &sessions {
            total += memory
                .load(&s.session_id)
                .await
                .map(|m| m.len())
                .unwrap_or(0);
        }
        total
    };
    out.push_str(&format!("synapse_memory_entries {}\n", memory_entries));

    (
        axum::http::StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        out,
    )
}
