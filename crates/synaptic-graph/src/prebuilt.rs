use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use synaptic_core::{ChatModel, ChatRequest, Message, SynapseError, Tool, ToolDefinition};
use synaptic_tools::{SerialToolExecutor, ToolRegistry};

use crate::builder::StateGraph;
use crate::checkpoint::Checkpointer;
use crate::compiled::CompiledGraph;
use crate::node::Node;
use crate::state::MessageState;
use crate::tool_node::ToolNode;
use crate::END;

/// Prebuilt node that calls a ChatModel with the current messages.
struct ChatModelNode {
    model: Arc<dyn ChatModel>,
    tool_defs: Vec<ToolDefinition>,
    system_prompt: Option<String>,
}

#[async_trait]
impl Node<MessageState> for ChatModelNode {
    async fn process(&self, mut state: MessageState) -> Result<MessageState, SynapseError> {
        let mut messages = Vec::new();

        // If a system prompt is configured, prepend it
        if let Some(ref prompt) = self.system_prompt {
            messages.push(Message::system(prompt));
        }

        messages.extend(state.messages.clone());

        let request = ChatRequest::new(messages).with_tools(self.tool_defs.clone());
        let response = self.model.chat(request).await?;
        state.messages.push(response.message);
        Ok(state)
    }
}

/// Options for creating a ReAct agent with `create_react_agent_with_options`.
///
/// All fields are optional and have sensible defaults. Use `Default::default()`
/// for the simplest configuration, which behaves identically to `create_react_agent`.
#[derive(Default)]
pub struct ReactAgentOptions {
    /// Optional checkpointer for state persistence across invocations.
    pub checkpointer: Option<Arc<dyn Checkpointer>>,
    /// Node names that should interrupt BEFORE execution (human-in-the-loop).
    pub interrupt_before: Vec<String>,
    /// Node names that should interrupt AFTER execution (human-in-the-loop).
    pub interrupt_after: Vec<String>,
    /// Optional system prompt to prepend to messages before calling the model.
    pub system_prompt: Option<String>,
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
    create_react_agent_with_options(model, tools, ReactAgentOptions::default())
}

/// Create a prebuilt ReAct agent graph with additional configuration options.
///
/// This is the extended version of [`create_react_agent`] that accepts
/// a [`ReactAgentOptions`] struct for configuring checkpointing,
/// interrupts, and system prompts.
///
/// The graph has two nodes:
/// - "agent": calls the ChatModel with messages and tool definitions
/// - "tools": executes any tool calls from the agent's response
///
/// Routing: if the agent returns tool calls, route to "tools"; otherwise route to END.
///
/// # Example
///
/// ```ignore
/// use std::sync::Arc;
/// use synaptic_graph::{create_react_agent_with_options, ReactAgentOptions, MemorySaver};
///
/// let options = ReactAgentOptions {
///     checkpointer: Some(Arc::new(MemorySaver::new())),
///     system_prompt: Some("You are a helpful assistant.".to_string()),
///     interrupt_before: vec!["tools".to_string()],
///     ..Default::default()
/// };
///
/// let graph = create_react_agent_with_options(model, tools, options)?;
/// ```
pub fn create_react_agent_with_options(
    model: Arc<dyn ChatModel>,
    tools: Vec<Arc<dyn Tool>>,
    options: ReactAgentOptions,
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

    let agent_node = ChatModelNode {
        model,
        tool_defs,
        system_prompt: options.system_prompt,
    };
    let tool_node = ToolNode::new(executor);

    let mut builder = StateGraph::new()
        .add_node("agent", agent_node)
        .add_node("tools", tool_node)
        .set_entry_point("agent")
        .add_conditional_edges_with_path_map(
            "agent",
            |state: &MessageState| {
                if let Some(last) = state.last_message() {
                    if !last.tool_calls().is_empty() {
                        return "tools".to_string();
                    }
                }
                END.to_string()
            },
            HashMap::from([
                ("tools".to_string(), "tools".to_string()),
                (END.to_string(), END.to_string()),
            ]),
        )
        .add_edge("tools", "agent");

    if !options.interrupt_before.is_empty() {
        builder = builder.interrupt_before(options.interrupt_before);
    }
    if !options.interrupt_after.is_empty() {
        builder = builder.interrupt_after(options.interrupt_after);
    }

    let mut graph = builder.compile()?;

    if let Some(checkpointer) = options.checkpointer {
        graph = graph.with_checkpointer(checkpointer);
    }

    Ok(graph)
}
