use async_trait::async_trait;
use synaptic_core::{Message, SynapseError};
use synaptic_tools::SerialToolExecutor;

use crate::node::Node;
use crate::state::MessageState;

/// Prebuilt node that executes tool calls from the last AI message in state.
pub struct ToolNode {
    executor: SerialToolExecutor,
}

impl ToolNode {
    pub fn new(executor: SerialToolExecutor) -> Self {
        Self { executor }
    }
}

#[async_trait]
impl Node<MessageState> for ToolNode {
    async fn process(&self, mut state: MessageState) -> Result<MessageState, SynapseError> {
        let last = state
            .last_message()
            .ok_or_else(|| SynapseError::Graph("no messages in state".to_string()))?;

        let tool_calls = last.tool_calls().to_vec();
        if tool_calls.is_empty() {
            return Ok(state);
        }

        for call in &tool_calls {
            let result = self
                .executor
                .execute(&call.name, call.arguments.clone())
                .await?;
            state
                .messages
                .push(Message::tool(result.to_string(), &call.id));
        }

        Ok(state)
    }
}
