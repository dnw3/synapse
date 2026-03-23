use axum::extract::State;
use axum::response::Json;
use axum::routing::get;
use axum::Router;
use serde::Serialize;
use serde_json::Value;

use crate::gateway::state::AppState;

pub fn routes() -> Router<AppState> {
    Router::new().route("/dashboard/plugins", get(get_plugins))
}

// ---------------------------------------------------------------------------
// GET /api/dashboard/plugins
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct ServiceInfo {
    id: String,
    status: String,
}

#[derive(Serialize)]
struct PluginResponse {
    name: String,
    version: String,
    description: String,
    author: Option<String>,
    license: Option<String>,
    source: String,
    enabled: bool,
    slot: Option<String>,
    capabilities: Vec<String>,
    health: String,
    tools: Vec<String>,
    interceptors: Vec<String>,
    subscribers: Vec<String>,
    services: Vec<ServiceInfo>,
}

/// Load disabled plugin names from persistent state file.
fn load_disabled_plugins() -> Vec<String> {
    let path = dirs::home_dir()
        .unwrap_or_default()
        .join(".synapse/plugins/state.json");
    std::fs::read_to_string(&path)
        .ok()
        .and_then(|data| serde_json::from_str::<Value>(&data).ok())
        .and_then(|v| v["disabled"].as_array().cloned())
        .map(|arr| {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        })
        .unwrap_or_default()
}

#[derive(Serialize)]
struct PluginsListResponse {
    plugins: Vec<PluginResponse>,
}

async fn get_plugins(State(state): State<AppState>) -> Json<PluginsListResponse> {
    let disabled = load_disabled_plugins();

    let registry = state.infra.plugin_registry.read().await;
    let mut plugins = Vec::new();

    for m in registry.plugins() {
        let regs = registry.plugin_registrations(&m.name);
        let caps: Vec<String> = m
            .capabilities
            .iter()
            .map(|c| format!("{:?}", c).to_lowercase())
            .collect();
        let slot = m.slot.as_ref().map(|s| format!("{:?}", s).to_lowercase());
        let service_ids = regs.map(|r| r.services.clone()).unwrap_or_default();

        let enabled = !disabled.contains(&m.name);
        let source = if m.name.starts_with("builtin-") || m.name.starts_with("memory-") {
            "builtin"
        } else {
            "external"
        };

        // Async health checks — now possible with tokio::RwLock
        let mut services_info = Vec::new();
        let mut all_healthy = true;
        let mut has_services = false;
        for svc_id in &service_ids {
            if let Some(svc) = registry.services().iter().find(|s| s.id() == svc_id) {
                has_services = true;
                let healthy = svc.health_check().await;
                if !healthy {
                    all_healthy = false;
                }
                services_info.push(ServiceInfo {
                    id: svc_id.clone(),
                    status: if healthy { "running" } else { "stopped" }.to_string(),
                });
            } else {
                services_info.push(ServiceInfo {
                    id: svc_id.clone(),
                    status: "unknown".to_string(),
                });
            }
        }

        let health = if !has_services {
            "unknown"
        } else if all_healthy {
            "healthy"
        } else {
            "error"
        };

        plugins.push(PluginResponse {
            name: m.name.clone(),
            version: m.version.clone(),
            description: m.description.clone(),
            author: m.author.clone(),
            license: m.license.clone(),
            source: source.to_string(),
            enabled,
            slot,
            capabilities: caps,
            health: health.to_string(),
            tools: regs.map(|r| r.tools.clone()).unwrap_or_default(),
            interceptors: regs.map(|r| r.interceptors.clone()).unwrap_or_default(),
            subscribers: regs.map(|r| r.subscribers.clone()).unwrap_or_default(),
            services: services_info,
        });
    }

    Json(PluginsListResponse { plugins })
}
