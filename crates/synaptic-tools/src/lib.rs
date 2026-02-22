pub mod brave;
pub mod calculator;
pub mod duckduckgo;
mod handle_error;
pub mod jina_reader;
mod parallel_executor;
mod return_direct;
pub mod wikipedia;

pub use brave::BraveSearchTool;
pub use calculator::CalculatorTool;
pub use duckduckgo::DuckDuckGoTool;
pub use handle_error::HandleErrorTool;
pub use jina_reader::JinaReaderTool;
pub use parallel_executor::ParallelToolExecutor;
pub use return_direct::ReturnDirectTool;
pub use wikipedia::WikipediaTool;

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

use synaptic_core::{SynapticError, Tool};

/// Thread-safe registry for tool definitions and implementations, backed by `Arc<RwLock<HashMap>>`.
#[derive(Default, Clone)]
pub struct ToolRegistry {
    inner: Arc<RwLock<HashMap<String, Arc<dyn Tool>>>>,
}

impl ToolRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&self, tool: Arc<dyn Tool>) -> Result<(), SynapticError> {
        let mut guard = self
            .inner
            .write()
            .map_err(|e| SynapticError::Tool(format!("registry lock poisoned: {e}")))?;
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
    ) -> Result<serde_json::Value, SynapticError> {
        let tool = self
            .registry
            .get(tool_name)
            .ok_or_else(|| SynapticError::ToolNotFound(tool_name.to_string()))?;
        tool.call(args).await
    }
}
