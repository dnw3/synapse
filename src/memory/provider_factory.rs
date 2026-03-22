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

    // Use the same factory registry as the gateway — no match arms needed.
    let factory_registry = crate::plugins::registry::default_registry();

    // For "memory-native" with LTM, we need special handling since the factory
    // doesn't have access to the LTM instance. Build it directly.
    if memory_slot == "memory-native" {
        return if let Some(ltm) = ltm {
            tracing::info!("using Native memory provider (LTM)");
            Arc::new(crate::memory::NativeMemoryProvider::new(ltm))
        } else {
            tracing::warn!("no LTM available, using no-op native provider");
            Arc::new(crate::memory::NativeMemoryProvider::new_noop())
        };
    }

    // For non-native plugins, use the factory to create a temporary Plugin,
    // register it, and extract the memory provider.
    let plugin_config = config
        .plugins
        .entries
        .get(memory_slot)
        .map(|e| e.config.clone())
        .unwrap_or_default();

    match factory_registry.create(memory_slot, plugin_config) {
        Some(_plugin) => {
            // The factory creates a Plugin, but we need the MemoryProvider directly.
            // For REPL path: construct the provider directly based on known types.
            // This is acceptable since REPL is a thin fallback path.
            if memory_slot == "memory-viking" {
                let vc = config
                    .plugins
                    .entries
                    .get("memory-viking")
                    .map(|e| e.config.clone())
                    .unwrap_or_default();
                let viking_config: crate::memory::VikingConfig =
                    serde_json::from_value(vc).unwrap_or_default();
                tracing::info!(url = %viking_config.url, "using Viking memory provider (REPL)");
                Arc::new(crate::memory::VikingMemoryProvider::new(viking_config))
            } else {
                tracing::warn!(
                    plugin = memory_slot,
                    "REPL fallback: unknown plugin, using noop"
                );
                Arc::new(crate::memory::NativeMemoryProvider::new_noop())
            }
        }
        None => {
            tracing::warn!(plugin = memory_slot, "unknown memory plugin, using noop");
            Arc::new(crate::memory::NativeMemoryProvider::new_noop())
        }
    }
}
