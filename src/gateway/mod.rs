mod api;
pub mod auth;
pub mod client;
pub mod error;
pub mod exec_approvals;
mod metrics;
pub mod nodes;
mod openai_compat;
pub mod presence;
mod request_id;
pub mod rpc;
mod sse;
pub mod state;
mod terminal;
pub mod webhooks;
mod ws;

use std::sync::Arc;
use std::time::Instant;

use axum::http::StatusCode;
use axum::middleware;
use axum::response::IntoResponse;
use colored::Colorize;
use serde::Serialize;
use tokio::sync::Mutex;

use crate::config::SynapseConfig;

/// Start the web server.
pub async fn run_server(
    config: &SynapseConfig,
    host: &str,
    port: u16,
) -> Result<(), Box<dyn std::error::Error>> {
    run_server_with_log_buffer(config, host, port, None).await
}

/// Start the web server with an optional shared log buffer.
pub async fn run_server_with_log_buffer(
    config: &SynapseConfig,
    host: &str,
    port: u16,
    log_buffer: Option<crate::logging::LogBuffer>,
) -> Result<(), Box<dyn std::error::Error>> {
    let app_state = if let Some(buf) = log_buffer {
        state::AppState::with_log_buffer(config, buf).await?
    } else {
        state::AppState::new(config).await?
    };

    // Build the main API router with auth middleware on protected routes
    let protected_api = api::create_router(app_state.clone())
        .merge(webhooks::routes().with_state(app_state.clone()))
        .merge(terminal::routes().with_state(app_state.clone()))
        .layer(middleware::from_fn_with_state(
            app_state.clone(),
            auth::require_auth,
        ));

    // Auth routes (not protected) + health endpoint
    let public_routes = auth::auth_router().with_state(app_state.clone());

    let health_state = app_state.clone();
    let health_route = axum::Router::new().route(
        "/health",
        axum::routing::get(move || health_handler(health_state)),
    );

    // Rate limiting on API routes
    let rate_limit_capacity = config
        .rate_limit
        .as_ref()
        .map(|rl| rl.capacity)
        .unwrap_or(60.0);
    let rate_limit_refill = config
        .rate_limit
        .as_ref()
        .map(|rl| rl.refill_rate)
        .unwrap_or(10.0);
    let limiter = RateLimiter::new(rate_limit_capacity, rate_limit_refill);
    let protected_api = protected_api.layer(middleware::from_fn_with_state(
        limiter,
        rate_limit_middleware,
    ));

    // Request metrics tracking layer (wraps all routes)
    let metrics_state = app_state.request_metrics.clone();

    let app = protected_api
        .merge(public_routes)
        .merge(health_route)
        .merge(metrics::routes().with_state(app_state.clone()))
        .merge(sse::routes().with_state(app_state.clone()))
        .merge(openai_compat::routes().with_state(app_state.clone()))
        .merge(crate::acp::server::routes().with_state(app_state.clone()))
        .merge(ws::ws_router(app_state))
        .layer(middleware::from_fn_with_state(
            metrics_state,
            request_metrics_middleware,
        ));

    // Serve uploaded files from data/uploads/
    let uploads_dir = std::path::Path::new("data/uploads");
    let _ = std::fs::create_dir_all(uploads_dir);
    let app = app.nest_service("/uploads", tower_http::services::ServeDir::new(uploads_dir));

    // Serve static files from web/dist if available
    let app = {
        let dist_dir = std::path::Path::new("web/dist");
        if dist_dir.exists() {
            app.fallback_service(tower_http::services::ServeDir::new(dist_dir).fallback(
                tower_http::services::ServeFile::new(dist_dir.join("index.html")),
            ))
        } else {
            app.fallback(|| async {
                (
                    axum::http::StatusCode::OK,
                    axum::response::Html(
                        "<h1>Synapse</h1><p>Web UI not built. Run <code>cd web && npm install && npm run build</code></p>",
                    ),
                )
            })
        }
    };

    // Request tracing — outermost layer: assigns request_id, logs completion
    let app = app.layer(middleware::from_fn(request_id::request_tracing_middleware));

    // CORS — configurable origins (default: permissive for dev)
    let app = app.layer(tower_http::cors::CorsLayer::permissive());

    let addr = format!("{}:{}", host, port);
    eprintln!(
        "{} http://{}",
        "Synapse server listening on".green().bold(),
        addr.cyan()
    );

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

// ---------------------------------------------------------------------------
// Rate limiting (simple token bucket)
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct RateLimiter {
    state: Arc<Mutex<(f64, Instant)>>, // (tokens, last_refill)
    capacity: f64,
    refill_rate: f64,
}

impl RateLimiter {
    fn new(capacity: f64, refill_rate: f64) -> Self {
        Self {
            state: Arc::new(Mutex::new((capacity, Instant::now()))),
            capacity,
            refill_rate,
        }
    }

    async fn try_acquire(&self) -> bool {
        let mut state = self.state.lock().await;
        let (tokens, last_refill) = &mut *state;

        // Refill tokens based on elapsed time
        let elapsed = last_refill.elapsed().as_secs_f64();
        *tokens = (*tokens + elapsed * self.refill_rate).min(self.capacity);
        *last_refill = Instant::now();

        if *tokens >= 1.0 {
            *tokens -= 1.0;
            true
        } else {
            false
        }
    }
}

async fn rate_limit_middleware(
    axum::extract::State(limiter): axum::extract::State<RateLimiter>,
    request: axum::http::Request<axum::body::Body>,
    next: middleware::Next,
) -> impl IntoResponse {
    if limiter.try_acquire().await {
        next.run(request).await.into_response()
    } else {
        (
            StatusCode::TOO_MANY_REQUESTS,
            "Too many requests — please slow down",
        )
            .into_response()
    }
}

// ---------------------------------------------------------------------------
// Request metrics tracking
// ---------------------------------------------------------------------------

async fn request_metrics_middleware(
    axum::extract::State(metrics): axum::extract::State<state::RequestMetrics>,
    request: axum::http::Request<axum::body::Body>,
    next: middleware::Next,
) -> impl IntoResponse {
    let method = request.method().to_string();
    let path = request.uri().path().to_string();
    let start = Instant::now();

    let response = next.run(request).await;

    let status = response.status().as_u16();
    let duration = start.elapsed().as_secs_f64();

    // Record request count
    {
        let mut reqs = metrics.requests.write().await;
        *reqs
            .entry((method.clone(), path.clone(), status))
            .or_insert(0) += 1;
    }

    // Record request duration
    {
        let mut durs = metrics.durations.write().await;
        let entry = durs.entry((method, path)).or_insert((0, 0.0));
        entry.0 += 1;
        entry.1 += duration;
    }

    response
}

// ---------------------------------------------------------------------------
// Health endpoint
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct HealthResponse {
    status: String,
    uptime_secs: u64,
    auth_enabled: bool,
}

async fn health_handler(state: state::AppState) -> axum::Json<HealthResponse> {
    let uptime = state.started_at.elapsed().as_secs();
    let auth_enabled = state
        .auth
        .as_ref()
        .map(|a| a.config.enabled)
        .unwrap_or(false);

    axum::Json(HealthResponse {
        status: "ok".to_string(),
        uptime_secs: uptime,
        auth_enabled,
    })
}
