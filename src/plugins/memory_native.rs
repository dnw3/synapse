//! memory-native plugin — registers NativeMemoryProvider + LTM tools.

use async_trait::async_trait;
use std::sync::Arc;
use synaptic::core::SynapticError;
use synaptic::plugin::{PluginApi, PluginCapability, PluginManifest, PluginSlot};

use crate::memory::{LongTermMemory, NativeMemoryProvider};

pub struct NativeMemoryPlugin {
    ltm: Option<Arc<LongTermMemory>>,
}

impl NativeMemoryPlugin {
    pub fn new(ltm: Option<Arc<LongTermMemory>>) -> Self {
        Self { ltm }
    }
}

#[async_trait]
impl synaptic::plugin::Plugin for NativeMemoryPlugin {
    fn manifest(&self) -> PluginManifest {
        PluginManifest {
            name: "memory-native".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            description: "Native long-term memory with embeddings and keyword search".into(),
            author: Some("synapse".into()),
            license: None,
            capabilities: vec![PluginCapability::Memory, PluginCapability::Tools],
            slot: Some(PluginSlot::Memory),
        }
    }

    async fn register(&self, api: &mut PluginApi<'_>) -> Result<(), SynapticError> {
        let provider = Arc::new(if let Some(ref ltm) = self.ltm {
            NativeMemoryProvider::new(ltm.clone())
        } else {
            NativeMemoryProvider::new_noop()
        });

        api.register_memory(provider.clone());
        api.register_tool(crate::tools::MemorySearchTool::new(provider));
        if let Some(ref ltm) = self.ltm {
            api.register_tool(crate::tools::MemoryGetTool::new(ltm.clone()));
            api.register_tool(crate::tools::MemorySaveTool::new(ltm.clone()));
            api.register_tool(crate::tools::MemoryForgetTool::new(ltm.clone()));
        }

        Ok(())
    }
}
