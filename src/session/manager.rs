use std::path::PathBuf;
use std::sync::Arc;

use synaptic::core::Store;
use synaptic::session::SessionManager;
use synaptic::store::FileStore;

use crate::config::SynapseConfig;

/// Build a SessionManager backed by FileStore for the configured sessions directory.
///
/// If a gateway shared store URL is configured, logs that shared store mode is active.
pub fn build_session_manager(config: &SynapseConfig) -> SessionManager {
    // Log gateway shared store configuration
    if let Some(ref gw) = config.gateway {
        if let Some(ref url) = gw.shared_store_url {
            let instance = gw.instance_id.as_deref().unwrap_or("default");
            if url.starts_with("redis://") {
                tracing::info!(backend = "redis", instance = %instance, "Shared store mode configured");
            } else if url.starts_with("postgres://") {
                tracing::info!(backend = "postgres", instance = %instance, "Shared store mode configured");
            } else {
                tracing::warn!(url = %url, "Unknown shared store URL scheme");
            }
        }
    }

    let sessions_dir = PathBuf::from(&config.base.paths.sessions_dir);
    let store: Arc<dyn Store> = Arc::new(FileStore::new(sessions_dir));
    SessionManager::new(store)
}
