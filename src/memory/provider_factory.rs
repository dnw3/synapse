use std::sync::Arc;

use synaptic::memory::MemoryProvider;

use crate::config::SynapseConfig;
use crate::memory::LongTermMemory;

/// Build the configured [`MemoryProvider`] based on [`SynapseConfig::memory`].
///
/// - `"viking"` → [`VikingMemoryProvider`](crate::memory::VikingMemoryProvider) backed by
///   the OpenViking REST API.
/// - `"native"` (default) → [`NativeMemoryProvider`](crate::memory::NativeMemoryProvider)
///   backed by the local [`LongTermMemory`] store.  If `ltm` is `None`, a no-op
///   provider is returned that yields empty results.
pub fn build_memory_provider(
    config: &SynapseConfig,
    ltm: Option<Arc<LongTermMemory>>,
) -> Arc<dyn MemoryProvider> {
    let provider_name = &config.memory.memory_provider;

    match provider_name.as_str() {
        "viking" => {
            let viking_config = config.memory.viking.clone().unwrap_or_default();
            tracing::info!(url = %viking_config.url, "using Viking memory provider");
            Arc::new(crate::memory::VikingMemoryProvider::new(viking_config))
        }
        _ => {
            // Default: native
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
