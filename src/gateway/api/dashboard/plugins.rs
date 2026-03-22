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

async fn get_plugins(State(state): State<AppState>) -> Json<Vec<PluginResponse>> {
    let disabled = load_disabled_plugins();

    let plugins: Vec<PluginResponse> = {
        let registry = state.plugin_registry.read().unwrap();
        registry
            .plugins()
            .iter()
            .map(|m| {
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

                PluginResponse {
                    name: m.name.clone(),
                    version: m.version.clone(),
                    description: m.description.clone(),
                    author: m.author.clone(),
                    license: m.license.clone(),
                    source: source.to_string(),
                    enabled,
                    slot,
                    capabilities: caps,
                    health: "unknown".to_string(),
                    tools: regs.map(|r| r.tools.clone()).unwrap_or_default(),
                    interceptors: regs.map(|r| r.interceptors.clone()).unwrap_or_default(),
                    subscribers: regs.map(|r| r.subscribers.clone()).unwrap_or_default(),
                    services: service_ids
                        .iter()
                        .map(|id| ServiceInfo {
                            id: id.clone(),
                            status: "unknown".to_string(),
                        })
                        .collect(),
                }
            })
            .collect()
    };

    Json(plugins)
}
