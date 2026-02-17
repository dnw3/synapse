use serde_json::Value;
use synapse_core::SynapseError;

use crate::ToolRegistry;

/// Executes multiple tool calls concurrently using `futures::future::join_all`.
pub struct ParallelToolExecutor {
    registry: ToolRegistry,
}

impl ParallelToolExecutor {
    /// Create a new parallel tool executor backed by the given registry.
    pub fn new(registry: ToolRegistry) -> Self {
        Self { registry }
    }

    /// Execute all tool calls concurrently and return results in the same order.
    ///
    /// Each element in `calls` is a `(tool_name, args)` pair.
    /// Results are returned in the same order as the input.
    pub async fn execute_all(
        &self,
        calls: Vec<(String, Value)>,
    ) -> Vec<Result<Value, SynapseError>> {
        let futures: Vec<_> = calls
            .into_iter()
            .map(|(name, args)| {
                let registry = self.registry.clone();
                async move {
                    let tool = registry
                        .get(&name)
                        .ok_or_else(|| SynapseError::ToolNotFound(name))?;
                    tool.call(args).await
                }
            })
            .collect();

        futures::future::join_all(futures).await
    }
}
