mod api;
pub mod auth;
pub mod channel_health;
pub mod channel_manager;
pub mod client;
pub mod error;
pub mod exec_approvals;
pub mod messages;
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
#[allow(dead_code)]
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

    // Clone broadcaster and channel_manager before app_state is consumed by ws_router
    let shutdown_broadcaster = app_state.broadcaster.clone();
    let channel_manager = app_state.channel_manager.clone();

    let app = protected_api
        .merge(public_routes)
        .merge(health_route)
        .merge(metrics::routes().with_state(app_state.clone()))
        .merge(sse::routes().with_state(app_state.clone()))
        .merge(openai_compat::routes().with_state(app_state.clone()))
        .merge(crate::acp::server::routes().with_state(app_state.clone()))
        .merge(ws::ws_router(app_state.clone()))
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

    // Register the gateway itself as a presence entry so the dashboard shows it
    {
        let host_name = std::process::Command::new("hostname")
            .output()
            .ok()
            .and_then(|o| String::from_utf8(o.stdout).ok())
            .map(|s| s.trim().to_string())
            .unwrap_or_default();
        let entry = crate::gateway::presence::PresenceEntry {
            key: String::new(),
            host: Some(host_name),
            ip: None,
            version: Some(env!("CARGO_PKG_VERSION").to_string()),
            platform: Some(std::env::consts::OS.to_string()),
            device_family: None,
            model_identifier: None,
            mode: Some("serve".to_string()),
            reason: Some("gateway".to_string()),
            device_id: None,
            instance_id: None,
            roles: vec!["gateway".to_string()],
            scopes: vec![],
            text: "Synapse Gateway".to_string(),
            ts: crate::gateway::presence::now_ms(),
        };
        app_state.presence.write().await.upsert(entry);
    }

    // Spawn enabled bot adapters as background tasks within the gateway process.
    // Each adapter runs its own event loop (e.g. Lark long-connection, Telegram
    // polling) and auto-reconnects on failure with exponential backoff.
    spawn_channel_adapters(config, channel_manager.clone(), &app_state);

    let health_monitor = channel_health::ChannelHealthMonitor::new(
        channel_manager,
        channel_health::HealthMonitorConfig::default(),
    );
    tokio::spawn(health_monitor.run());

    let listener = tokio::net::TcpListener::bind(&addr).await?;

    // Graceful shutdown: broadcast shutdown event, then stop accepting connections
    let broadcaster = shutdown_broadcaster;
    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = tokio::signal::ctrl_c().await;
            eprintln!("\n{}", "Shutting down gracefully...".yellow().bold());
            // Broadcast shutdown event to all connected clients
            broadcaster
                .broadcast("shutdown", serde_json::json!({"reason": "server_shutdown"}))
                .await;
            // Give in-flight RPCs a moment to complete
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;
        })
        .await?;

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

// ---------------------------------------------------------------------------
// Channel adapter spawning — start enabled bot adapters within the gateway
// ---------------------------------------------------------------------------

fn spawn_channel_adapters(
    config: &SynapseConfig,
    manager: Arc<channel_manager::ChannelAdapterManager>,
    app_state: &state::AppState,
) {
    // Each adapter: iterate per-account configs, skip disabled, spawn one task each.
    // Adapters auto-reconnect internally with backoff. If startup fails, log and
    // continue — the gateway stays up even if a channel adapter can't connect.

    #[cfg(feature = "bot-lark")]
    for account in &config.lark {
        if !account.enabled {
            continue;
        }

        // Register Lark approve notifier (for DM pairing approval notifications)
        if let Ok(secret) = crate::config::bot::resolve_secret(
            account.app_secret.as_deref(),
            account.app_secret_env.as_deref(),
            "Lark app secret (notifier)",
        ) {
            let notifier_client = synaptic::lark::LarkBotClient::new(
                synaptic::lark::LarkConfig::new(&account.app_id, &secret),
            );
            let notifier = Arc::new(crate::channels::adapters::lark::LarkApproveNotifier {
                client: notifier_client,
            });
            app_state.approve_notifiers.register("lark", notifier);
            tracing::info!(channel = "lark", "registered DM pairing approval notifier");
        }

        let cfg = config.clone();
        let account_id = account.account_id.clone();
        let mgr = manager.clone();
        let handle = Arc::new(channel_manager::ChannelStatusHandleImpl::new(
            "lark",
            &account_id,
        ));
        let status_handle: Arc<dyn synaptic::ChannelStatusHandle> = handle.clone();
        let task = tokio::spawn(async move {
            tracing::info!(channel = "lark", account_id = %account_id, "starting channel adapter");
            loop {
                match crate::channels::adapters::lark::run(&cfg, None, Some(status_handle.clone()))
                    .await
                {
                    Ok(()) => {
                        tracing::info!(channel = "lark", account_id = %account_id, "adapter exited, restarting in 5s");
                    }
                    Err(e) => {
                        tracing::error!(channel = "lark", account_id = %account_id, error = %e, "adapter failed, restarting in 5s");
                    }
                }
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        });
        let mgr2 = mgr.clone();
        let aid = account.account_id.clone();
        tokio::spawn(async move {
            mgr2.register("lark", &aid, task, None, handle).await;
        });
    }

    #[cfg(feature = "bot-telegram")]
    for account in &config.telegram {
        if !account.enabled {
            continue;
        }
        let cfg = config.clone();
        let account_id = account.account_id.clone();
        let mgr = manager.clone();
        let handle = Arc::new(channel_manager::ChannelStatusHandleImpl::new(
            "telegram",
            &account_id,
        ));
        let task = tokio::spawn(async move {
            tracing::info!(channel = "telegram", account_id = %account_id, "starting channel adapter");
            if let Err(e) = crate::channels::adapters::telegram::run(&cfg, None).await {
                tracing::error!(channel = "telegram", account_id = %account_id, error = %e, "adapter failed");
            }
        });
        let aid = account.account_id.clone();
        tokio::spawn(async move {
            mgr.register("telegram", &aid, task, None, handle).await;
        });
    }

    #[cfg(feature = "bot-discord")]
    for account in &config.discord {
        if !account.enabled {
            continue;
        }
        let cfg = config.clone();
        let account_id = account.account_id.clone();
        let mgr = manager.clone();
        let handle = Arc::new(channel_manager::ChannelStatusHandleImpl::new(
            "discord",
            &account_id,
        ));
        let task = tokio::spawn(async move {
            tracing::info!(channel = "discord", account_id = %account_id, "starting channel adapter");
            if let Err(e) = crate::channels::adapters::discord::run(&cfg, None).await {
                tracing::error!(channel = "discord", account_id = %account_id, error = %e, "adapter failed");
            }
        });
        let aid = account.account_id.clone();
        tokio::spawn(async move {
            mgr.register("discord", &aid, task, None, handle).await;
        });
    }

    #[cfg(feature = "bot-slack")]
    for account in &config.slack {
        if !account.enabled {
            continue;
        }
        let cfg = config.clone();
        let account_id = account.account_id.clone();
        let mgr = manager.clone();
        let handle = Arc::new(channel_manager::ChannelStatusHandleImpl::new(
            "slack",
            &account_id,
        ));
        let task = tokio::spawn(async move {
            tracing::info!(channel = "slack", account_id = %account_id, "starting channel adapter");
            if let Err(e) = crate::channels::adapters::slack::run(&cfg, None).await {
                tracing::error!(channel = "slack", account_id = %account_id, error = %e, "adapter failed");
            }
        });
        let aid = account.account_id.clone();
        tokio::spawn(async move {
            mgr.register("slack", &aid, task, None, handle).await;
        });
    }
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
