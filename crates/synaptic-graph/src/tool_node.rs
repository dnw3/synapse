use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use synaptic_core::{Message, RuntimeAwareTool, Store, SynapticError, ToolRuntime};
use synaptic_middleware::{MiddlewareChain, ToolCallRequest, ToolCaller};
use synaptic_tools::SerialToolExecutor;

use crate::command::NodeOutput;
use crate::node::Node;
use crate::state::MessageState;

/// Wraps a `SerialToolExecutor` into a `ToolCaller` for the middleware chain.
struct BaseToolCaller {
    executor: SerialToolExecutor,
}

#[async_trait]
impl ToolCaller for BaseToolCaller {
    async fn call(&self, request: ToolCallRequest) -> Result<Value, SynapticError> {
        self.executor
            .execute(&request.call.name, request.call.arguments.clone())
            .await
    }
}

/// Prebuilt node that executes tool calls from the last AI message in state.
///
/// Supports both regular `Tool` and `RuntimeAwareTool` instances.
/// When a runtime-aware tool is registered, it receives the current graph
/// state, store reference, and tool call ID via [`ToolRuntime`].
pub struct ToolNode {
    executor: SerialToolExecutor,
    middleware: Option<Arc<MiddlewareChain>>,
    /// Optional store reference injected into RuntimeAwareTool calls.
    store: Option<Arc<dyn Store>>,
    /// Runtime-aware tools keyed by tool name.
    runtime_tools: HashMap<String, Arc<dyn RuntimeAwareTool>>,
}

impl ToolNode {
    pub fn new(executor: SerialToolExecutor) -> Self {
        Self {
            executor,
            middleware: None,
            store: None,
            runtime_tools: HashMap::new(),
        }
    }

    /// Create a ToolNode with middleware support.
    pub fn with_middleware(executor: SerialToolExecutor, middleware: Arc<MiddlewareChain>) -> Self {
        Self {
            executor,
            middleware: Some(middleware),
            store: None,
            runtime_tools: HashMap::new(),
        }
    }

    /// Set the store reference for runtime-aware tool injection.
    pub fn with_store(mut self, store: Arc<dyn Store>) -> Self {
        self.store = Some(store);
        self
    }

    /// Register a runtime-aware tool.
    ///
    /// When a tool call matches a registered runtime-aware tool by name,
    /// it will be called with a [`ToolRuntime`] containing the current
    /// graph state, store, and tool call ID.
    pub fn with_runtime_tool(mut self, tool: Arc<dyn RuntimeAwareTool>) -> Self {
        self.runtime_tools.insert(tool.name().to_string(), tool);
        self
    }
}

#[async_trait]
impl Node<MessageState> for ToolNode {
    async fn process(
        &self,
        mut state: MessageState,
    ) -> Result<NodeOutput<MessageState>, SynapticError> {
        let last = state
            .last_message()
            .ok_or_else(|| SynapticError::Graph("no messages in state".to_string()))?;

        let tool_calls = last.tool_calls().to_vec();
        if tool_calls.is_empty() {
            return Ok(state.into());
        }

        // Serialize current state for context injection
        let state_value = serde_json::to_value(&state).ok();

        for call in &tool_calls {
            // Check if this is a runtime-aware tool
            let result = if let Some(rt_tool) = self.runtime_tools.get(&call.name) {
                let runtime = ToolRuntime {
                    store: self.store.clone(),
                    stream_writer: None,
                    state: state_value.clone(),
                    tool_call_id: call.id.clone(),
                    config: None,
                };
                rt_tool
                    .call_with_runtime(call.arguments.clone(), runtime)
                    .await?
            } else {
                // Regular tool execution
                if let Some(ref chain) = self.middleware {
                    let request = ToolCallRequest { call: call.clone() };
                    let base = BaseToolCaller {
                        executor: self.executor.clone(),
                    };
                    chain.call_tool(request, &base).await?
                } else {
                    self.executor
                        .execute(&call.name, call.arguments.clone())
                        .await?
                }
            };
            state
                .messages
                .push(Message::tool(result.to_string(), &call.id));
        }

        Ok(state.into())
    }
}

/// Standard routing function: returns "tools" if last message has tool_calls, else END.
///
/// This is the standard condition function used with `add_conditional_edges`
/// to route between an agent node and a tools node.
pub fn tools_condition(state: &MessageState) -> String {
    if let Some(last) = state.last_message() {
        if !last.tool_calls().is_empty() {
            return "tools".to_string();
        }
    }
    crate::END.to_string()
}
