mod handle_error;
mod parallel_executor;
mod return_direct;

pub use handle_error::HandleErrorTool;
pub use parallel_executor::ParallelToolExecutor;
pub use return_direct::ReturnDirectTool;

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use synaptic_core::{SynapseError, Tool};

/// Thread-safe registry for tool definitions and implementations, backed by `Arc<RwLock<HashMap>>`.
#[derive(Default, Clone)]
pub struct ToolRegistry {
    inner: Arc<RwLock<HashMap<String, Arc<dyn Tool>>>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, tool: Arc<dyn Tool>) -> Result<(), SynapseError> {
        let mut guard = self
            .inner
            .write()
            .map_err(|e| SynapseError::Tool(format!("registry lock poisoned: {e}")))?;
        guard.insert(tool.name().to_string(), tool);
        Ok(())
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn Tool>> {
        let guard = self.inner.read().ok()?;
        guard.get(name).cloned()
    }
}

/// Executes tool calls sequentially, looking up tools in a `ToolRegistry`.
#[derive(Clone)]
pub struct SerialToolExecutor {
    registry: ToolRegistry,
}

impl SerialToolExecutor {
    pub fn new(registry: ToolRegistry) -> Self {
        Self { registry }
    }

    pub async fn execute(
        &self,
        tool_name: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, SynapseError> {
        let tool = self
            .registry
            .get(tool_name)
            .ok_or_else(|| SynapseError::ToolNotFound(tool_name.to_string()))?;
        tool.call(args).await
    }
}
