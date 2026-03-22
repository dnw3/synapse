use std::sync::Arc;

use synaptic::memory::MemoryProvider;

use crate::config::SynapseConfig;
use crate::memory::LongTermMemory;

/// Build a [`MemoryProvider`] based on `[plugins].slots.memory` config.
///
/// Used as fallback for REPL/CLI paths where the full plugin system isn't active.
/// The gateway uses `PluginRegistry.memory_slot` instead.
pub fn build_memory_provider(
    config: &SynapseConfig,
    ltm: Option<Arc<LongTermMemory>>,
) -> Arc<dyn MemoryProvider> {
    let memory_slot = config
        .plugins
        .slots
        .get("memory")
        .map(|s| s.as_str())
        .unwrap_or("memory-native");

    match memory_slot {
        "memory-viking" => {
            let plugin_config = config
                .plugins
                .entries
                .get("memory-viking")
                .map(|e| e.config.clone())
                .unwrap_or_default();
            let viking_config: crate::memory::VikingConfig =
                serde_json::from_value(plugin_config).unwrap_or_default();
            tracing::info!(url = %viking_config.url, "using Viking memory provider");
            Arc::new(crate::memory::VikingMemoryProvider::new(viking_config))
        }
        _ => {
            if let Some(ltm) = ltm {
                tracing::info!("using Native memory provider (LTM)");
                Arc::new(crate::memory::NativeMemoryProvider::new(ltm))
            } else {
                tracing::warn!("no LTM available, using no-op native provider");
                Arc::new(crate::memory::NativeMemoryProvider::new_noop())
            }
        }
    }
}
