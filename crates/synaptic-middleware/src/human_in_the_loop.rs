use std::collections::HashSet;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use synaptic_core::SynapticError;

use crate::{AgentMiddleware, ToolCallRequest, ToolCaller};

/// A callback that decides whether a tool call should proceed.
///
/// Return `Ok(true)` to approve, `Ok(false)` to reject (returns an
/// error message to the model), or `Err(...)` to abort.
#[async_trait]
pub trait ApprovalCallback: Send + Sync {
    async fn approve(&self, tool_name: &str, arguments: &Value) -> Result<bool, SynapticError>;
}

/// Pauses tool execution to request human approval.
///
/// When a tool call targets one of the configured tool names (or all
/// tools if the set is empty), the middleware invokes the
/// `ApprovalCallback`. If the callback returns `false`, the tool call
/// is replaced with an error message fed back to the model.
pub struct HumanInTheLoopMiddleware {
    callback: Arc<dyn ApprovalCallback>,
    /// Tool names that require approval. Empty means all tools.
    tools: HashSet<String>,
}

impl HumanInTheLoopMiddleware {
    /// Create middleware that requires approval for all tool calls.
    pub fn new(callback: Arc<dyn ApprovalCallback>) -> Self {
        Self {
            callback,
            tools: HashSet::new(),
        }
    }

    /// Create middleware that requires approval only for specific tools.
    pub fn for_tools(callback: Arc<dyn ApprovalCallback>, tools: Vec<String>) -> Self {
        Self {
            callback,
            tools: tools.into_iter().collect(),
        }
    }
}

#[async_trait]
impl AgentMiddleware for HumanInTheLoopMiddleware {
    async fn wrap_tool_call(
        &self,
        request: ToolCallRequest,
        next: &dyn ToolCaller,
    ) -> Result<Value, SynapticError> {
        let needs_approval = self.tools.is_empty() || self.tools.contains(&request.call.name);

        if needs_approval {
            let approved = self
                .callback
                .approve(&request.call.name, &request.call.arguments)
                .await?;

            if !approved {
                return Ok(Value::String(format!(
                    "Tool call '{}' was rejected by human review.",
                    request.call.name
                )));
            }
        }

        next.call(request).await
    }
}
