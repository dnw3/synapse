//! Unified agent execution runtime.
//!
//! Provides the [`AgentRuntime`] trait that abstracts over different
//! execution modes (streaming for REPL/task, invoke for bots).

use std::sync::Arc;

use async_trait::async_trait;
use futures::StreamExt;
use synaptic::core::{Message, SynapticError, TokenUsage};
use synaptic::graph::{CompiledGraph, MessageState, StreamMode};

/// Result of running an agent to completion.
pub struct AgentResult {
    /// All messages including the agent's responses.
    pub messages: Vec<Message>,
    /// The final text response extracted from the last AI message.
    pub response_text: String,
    /// Token usage, if available.
    pub usage: Option<TokenUsage>,
}

/// Agent execution runtime — unifies REPL, task, and bot execution modes.
#[async_trait]
pub trait AgentRuntime: Send + Sync {
    /// Run the agent with the given messages, returning the final response.
    async fn run(
        &self,
        agent: &CompiledGraph<MessageState>,
        messages: Vec<Message>,
    ) -> Result<AgentResult, SynapticError>;
}

/// Streaming mode: prints tool calls and chunks to terminal.
///
/// Used by REPL and task execution modes. Streams events and
/// renders them in real-time.
pub struct StreamingRuntime {
    /// Callback invoked for each new message batch during streaming.
    pub on_messages: Option<Arc<dyn Fn(&[Message], usize) -> usize + Send + Sync>>,
}

impl StreamingRuntime {
    pub fn new() -> Self {
        Self { on_messages: None }
    }

    pub fn with_renderer(
        on_messages: Arc<dyn Fn(&[Message], usize) -> usize + Send + Sync>,
    ) -> Self {
        Self {
            on_messages: Some(on_messages),
        }
    }
}

impl Default for StreamingRuntime {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl AgentRuntime for StreamingRuntime {
    async fn run(
        &self,
        agent: &CompiledGraph<MessageState>,
        messages: Vec<Message>,
    ) -> Result<AgentResult, SynapticError> {
        let initial_state = MessageState::with_messages(messages);
        let stream = agent.stream(initial_state, StreamMode::Values);
        tokio::pin!(stream);

        let mut displayed_count = 0usize;
        let mut final_messages = Vec::new();

        while let Some(event) = stream.next().await {
            match event {
                Ok(graph_event) => {
                    if let Some(ref renderer) = self.on_messages {
                        displayed_count = renderer(&graph_event.state.messages, displayed_count);
                    }
                    final_messages = graph_event.state.messages;
                }
                Err(e) => return Err(e),
            }
        }

        let response_text = extract_final_response(&final_messages);

        Ok(AgentResult {
            messages: final_messages,
            response_text,
            usage: None,
        })
    }
}

/// Invoke mode: non-streaming, returns full response.
///
/// Used by bot adapters. Runs the agent to completion and returns
/// the final response text.
pub struct InvokeRuntime;

#[async_trait]
impl AgentRuntime for InvokeRuntime {
    async fn run(
        &self,
        agent: &CompiledGraph<MessageState>,
        messages: Vec<Message>,
    ) -> Result<AgentResult, SynapticError> {
        let initial_state = MessageState::with_messages(messages);
        let result = agent.invoke(initial_state).await?;
        let final_state = result.into_state();

        let response_text = extract_final_response(&final_state.messages);

        Ok(AgentResult {
            messages: final_state.messages,
            response_text,
            usage: None,
        })
    }
}

/// Extract the final AI response text from the message list.
fn extract_final_response(messages: &[Message]) -> String {
    for msg in messages.iter().rev() {
        if msg.is_ai() {
            let content = msg.content();
            if !content.is_empty() {
                return content.to_string();
            }
        }
    }
    "I processed your request but have no text response.".to_string()
}
