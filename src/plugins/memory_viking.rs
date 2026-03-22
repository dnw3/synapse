//! memory-viking plugin — wires VikingMemoryProvider + MemorySearchTool + VikingService
//! into the plugin registry as a single, self-contained unit.

use std::sync::Arc;

use async_trait::async_trait;
use synaptic::core::SynapticError;
use synaptic::plugin::{
    Plugin, PluginApi, PluginCapability, PluginContext, PluginManifest, PluginSlot,
};

use crate::memory::{VikingConfig, VikingMemoryProvider};

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
        // Keep concrete type for Viking-specific tools
        let concrete_provider = Arc::new(VikingMemoryProvider::new(self.config.clone()));
        // Trait object for shared components
        let provider: Arc<dyn synaptic::memory::MemoryProvider> = concrete_provider.clone();

        // 1. Memory slot
        api.register_memory(provider.clone());

        // 2. Shared tool: memory_search
        api.register_tool(crate::tools::MemorySearchTool::new(provider.clone()));

        // 3. Viking-specific tools
        api.register_tool(super::viking_tools::VikingContentTool::new(
            concrete_provider.clone(),
        ));
        api.register_tool(super::viking_tools::VikingCommitMemoryTool::new(
            concrete_provider.clone(),
        ));

        // 4. Auto-recall interceptor (shared, faces dyn MemoryProvider)
        if self.config.auto_recall {
            api.register_interceptor(Arc::new(
                super::memory_recall::MemoryRecallInterceptor::new(
                    provider.clone(),
                    self.config.recall_limit,
                    self.config.recall_score_threshold,
                ),
            ));
            tracing::info!("memory-viking: auto-recall enabled");
        }

        // 5. Auto-capture subscriber (shared, faces dyn MemoryProvider)
        if self.config.auto_capture {
            api.register_event_subscriber(
                Arc::new(super::memory_capture::MemoryCaptureSubscriber::new(
                    provider.clone(),
                )),
                0, // default priority
            );
            tracing::info!("memory-viking: auto-capture enabled");
        }

        // 6. Managed service (VikingService)
        api.register_service(Box::new(super::viking_service::VikingService::new(
            self.config.clone(),
        )));

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
