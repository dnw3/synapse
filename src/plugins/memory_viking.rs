//! memory-viking plugin — wires VikingMemoryProvider + MemorySearchTool + VikingService
//! into the plugin registry as a single, self-contained unit.

use std::sync::Arc;

use async_trait::async_trait;
use synaptic::core::SynapticError;
use synaptic::plugin::{
    Plugin, PluginApi, PluginCapability, PluginContext, PluginManifest, PluginSlot,
};

use crate::memory::{VikingConfig, VikingMemoryProvider};

use super::viking_service::VikingService;

/// Built-in plugin that registers the Viking memory backend.
pub struct VikingMemoryPlugin {
    config: VikingConfig,
}

impl VikingMemoryPlugin {
    pub fn new(config: VikingConfig) -> Self {
        Self { config }
    }
}

#[async_trait]
impl Plugin for VikingMemoryPlugin {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            name: "memory-viking".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            description: "OpenViking-backed long-term memory provider with semantic search.".into(),
            author: None,
            license: None,
            capabilities: vec![
                PluginCapability::Memory,
                PluginCapability::Tools,
                PluginCapability::Services,
            ],
            slot: Some(PluginSlot::Memory),
        }
    }

    async fn register(&self, api: &mut PluginApi<'_>) -> Result<(), SynapticError> {
        let provider: Arc<dyn synaptic::memory::MemoryProvider> =
            Arc::new(VikingMemoryProvider::new(self.config.clone()));

        // Register the memory slot (exclusive — replaces any previous provider).
        api.register_memory(provider.clone());

        // Register the memory_search tool backed by this provider.
        api.register_tool(crate::tools::MemorySearchTool::new(provider));

        // Register the managed VikingService so the process is started/stopped
        // with the plugin lifecycle.
        api.register_service(Box::new(VikingService::new(self.config.clone())));

        Ok(())
    }

    /// `start()` and `stop()` are intentionally left as the default no-ops:
    /// service lifecycle (process management) is handled by `VikingService`.
    async fn start(&self, _ctx: PluginContext) -> Result<(), SynapticError> {
        Ok(())
    }

    async fn stop(&self) -> Result<(), SynapticError> {
        Ok(())
    }
}
