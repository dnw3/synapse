use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use synapse_core::{SynapseError, Tool};

/// A tool wrapper that signals the agent should return the tool's output directly
/// to the user without further LLM processing.
pub struct ReturnDirectTool {
    inner: Arc<dyn Tool>,
}

impl ReturnDirectTool {
    /// Wrap an existing tool so its output is returned directly to the user.
    pub fn new(inner: Arc<dyn Tool>) -> Self {
        Self { inner }
    }

    /// Returns `true`, indicating this tool's output should be returned directly.
    pub fn is_return_direct(&self) -> bool {
        true
    }
}

#[async_trait]
impl Tool for ReturnDirectTool {
    fn name(&self) -> &'static str {
        self.inner.name()
    }

    fn description(&self) -> &'static str {
        self.inner.description()
    }

    async fn call(&self, args: Value) -> Result<Value, SynapseError> {
        self.inner.call(args).await
    }
}
