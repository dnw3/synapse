pub mod bridge;
pub mod bundle;
pub mod config;
pub mod discovery;
pub mod manager;
pub mod memory_capture;
pub mod memory_native;
pub mod memory_recall;
pub mod memory_viking;
pub mod observability;
pub mod registry;
pub mod viking_service;
pub mod viking_tools;

use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

use synaptic::events::EventBus;
use synaptic::plugin::PluginRegistry;

use crate::memory::LongTermMemory;

/// Result of CLI plugin initialization — mirrors gateway's InfraBundle.
pub struct CliPluginBundle {
    pub event_bus: Arc<EventBus>,
    pub plugin_registry: Arc<RwLock<PluginRegistry>>,
    pub bundle_skills_dirs: Vec<PathBuf>,
}

/// Build a plugin registry for CLI/task/REPL paths (no gateway).
///
/// Creates an EventBus + PluginRegistry, registers the configured memory plugin
/// (with LTM for native, or VikingService for viking), discovers filesystem
/// plugins/bundles, and starts services.
pub async fn build_cli_plugins(
    config: &crate::config::SynapseConfig,
    ltm: Option<Arc<LongTermMemory>>,
) -> CliPluginBundle {
    let event_bus = Arc::new(EventBus::new());
    let plugin_registry = Arc::new(RwLock::new(PluginRegistry::new(event_bus.clone())));

    let data_root = dirs::home_dir()
        .unwrap_or_default()
        .join(".synapse/plugins");

    let mut mgr =
        manager::PluginManager::new(config.plugins.clone(), plugin_registry.clone(), data_root);

    // Register slot-assigned plugins (memory, etc.)
    let factory_reg = registry::default_registry();
    for (slot, plugin_name) in &config.plugins.slots {
        let plugin_config = config
            .plugins
            .entries
            .get(plugin_name)
            .map(|e| e.config.clone())
            .unwrap_or_default();

        let enabled = config
            .plugins
            .entries
            .get(plugin_name)
            .map(|e| e.enabled)
            .unwrap_or(true);

        if !enabled {
            tracing::info!(plugin = %plugin_name, slot = %slot, "plugin disabled in config");
            continue;
        }

        // Special case: memory-native needs Arc<LTM> which can't pass through JSON config
        if plugin_name == "memory-native" && slot == "memory" {
            mgr.add_builtin(Box::new(memory_native::NativeMemoryPlugin::new(
                ltm.clone(),
            )));
            continue;
        }

        match factory_reg.create(plugin_name, plugin_config) {
            Some(plugin) => {
                mgr.add_builtin(plugin);
                tracing::debug!(plugin = %plugin_name, slot = %slot, "queued slot plugin");
            }
            None => {
                tracing::warn!(
                    plugin = %plugin_name, slot = %slot,
                    "unknown plugin — not found in builtin registry"
                );
            }
        }
    }

    // If no memory slot configured, default to native
    if !config.plugins.slots.contains_key("memory") {
        mgr.add_builtin(Box::new(memory_native::NativeMemoryPlugin::new(ltm)));
    }

    mgr.load_state();
    if let Err(e) = mgr.load_all().await {
        tracing::warn!(error = %e, "failed to load CLI plugins");
    }
    if let Err(e) = mgr.discover_and_load().await {
        tracing::warn!(error = %e, "failed to discover external plugins");
    }
    mgr.start_services().await;

    let bundle_skills_dirs = mgr.bundle_skills_dirs.clone();

    CliPluginBundle {
        event_bus,
        plugin_registry,
        bundle_skills_dirs,
    }
}
