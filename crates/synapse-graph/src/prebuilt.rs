use std::sync::Arc;

use async_trait::async_trait;
use synapse_core::{ChatModel, ChatRequest, SynapseError, Tool, ToolDefinition};
use synapse_tools::{SerialToolExecutor, ToolRegistry};

use crate::builder::StateGraph;
use crate::compiled::CompiledGraph;
use crate::node::Node;
use crate::state::MessageState;
use crate::tool_node::ToolNode;
use crate::END;

/// Prebuilt node that calls a ChatModel with the current messages.
struct ChatModelNode {
    model: Arc<dyn ChatModel>,
    tool_defs: Vec<ToolDefinition>,
}

#[async_trait]
impl Node<MessageState> for ChatModelNode {
    async fn process(&self, mut state: MessageState) -> Result<MessageState, SynapseError> {
        let request = ChatRequest::new(state.messages.clone()).with_tools(self.tool_defs.clone());
        let response = self.model.chat(request).await?;
        state.messages.push(response.message);
        Ok(state)
    }
}

/// Create a prebuilt ReAct agent graph.
///
/// The graph has two nodes:
/// - "agent": calls the ChatModel with messages and tool definitions
/// - "tools": executes any tool calls from the agent's response
///
/// Routing: if the agent returns tool calls, route to "tools"; otherwise route to END.
pub fn create_react_agent(
    model: Arc<dyn ChatModel>,
    tools: Vec<Arc<dyn Tool>>,
) -> Result<CompiledGraph<MessageState>, SynapseError> {
    let tool_defs: Vec<ToolDefinition> = tools
        .iter()
        .map(|t| ToolDefinition {
            name: t.name().to_string(),
            description: t.description().to_string(),
            parameters: serde_json::json!({}),
        })
        .collect();

    let registry = ToolRegistry::new();
    for tool in tools {
        registry.register(tool)?;
    }
    let executor = SerialToolExecutor::new(registry);

    let agent_node = ChatModelNode { model, tool_defs };
    let tool_node = ToolNode::new(executor);

    StateGraph::new()
        .add_node("agent", agent_node)
        .add_node("tools", tool_node)
        .set_entry_point("agent")
        .add_conditional_edges("agent", |state: &MessageState| {
            if let Some(last) = state.last_message() {
                if !last.tool_calls().is_empty() {
                    return "tools".to_string();
                }
            }
            END.to_string()
        })
        .add_edge("tools", "agent")
        .compile()
}
