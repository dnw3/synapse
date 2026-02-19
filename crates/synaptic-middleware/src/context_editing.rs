use async_trait::async_trait;
use synaptic_core::{Message, SynapticError};

use crate::{AgentMiddleware, ModelRequest};

/// Strategy for editing context before model calls.
#[derive(Debug, Clone)]
pub enum ContextStrategy {
    /// Keep the last N messages (preserving system messages).
    LastN(usize),
    /// Remove tool call/result pairs from the history, keeping only
    /// human and AI content messages.
    StripToolCalls,
    /// Apply both: strip tool calls, then keep last N.
    StripAndTruncate(usize),
}

/// Manages conversation context by trimming or filtering messages
/// before each model invocation.
///
/// This is useful for keeping the context window manageable without
/// full summarization.
pub struct ContextEditingMiddleware {
    strategy: ContextStrategy,
}

impl ContextEditingMiddleware {
    pub fn new(strategy: ContextStrategy) -> Self {
        Self { strategy }
    }

    /// Keep last N messages, always preserving leading system messages.
    pub fn last_n(n: usize) -> Self {
        Self::new(ContextStrategy::LastN(n))
    }

    /// Strip tool call/result message pairs from history.
    pub fn strip_tool_calls() -> Self {
        Self::new(ContextStrategy::StripToolCalls)
    }

    fn apply_last_n(messages: &mut Vec<Message>, n: usize) {
        if messages.len() <= n {
            return;
        }

        // Preserve leading system messages
        let system_count = messages.iter().take_while(|m| m.is_system()).count();
        let non_system = &messages[system_count..];
        if non_system.len() <= n {
            return;
        }

        let keep_from = non_system.len() - n;
        let mut new_msgs: Vec<Message> = messages[..system_count].to_vec();
        new_msgs.extend_from_slice(&messages[system_count + keep_from..]);
        *messages = new_msgs;
    }

    fn apply_strip_tool_calls(messages: &mut Vec<Message>) {
        messages.retain(|m| {
            // Keep all non-tool messages, but strip AI messages that
            // contain only tool calls (no text content)
            if m.is_tool() {
                return false;
            }
            if m.is_ai() && !m.tool_calls().is_empty() && m.content().is_empty() {
                return false;
            }
            true
        });
    }
}

#[async_trait]
impl AgentMiddleware for ContextEditingMiddleware {
    async fn before_model(&self, request: &mut ModelRequest) -> Result<(), SynapticError> {
        match &self.strategy {
            ContextStrategy::LastN(n) => {
                Self::apply_last_n(&mut request.messages, *n);
            }
            ContextStrategy::StripToolCalls => {
                Self::apply_strip_tool_calls(&mut request.messages);
            }
            ContextStrategy::StripAndTruncate(n) => {
                Self::apply_strip_tool_calls(&mut request.messages);
                Self::apply_last_n(&mut request.messages, *n);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn last_n_preserves_system() {
        let mut msgs = vec![
            Message::system("sys"),
            Message::human("1"),
            Message::ai("2"),
            Message::human("3"),
            Message::ai("4"),
        ];
        ContextEditingMiddleware::apply_last_n(&mut msgs, 2);
        assert_eq!(msgs.len(), 3); // sys + last 2
        assert!(msgs[0].is_system());
        assert_eq!(msgs[1].content(), "3");
        assert_eq!(msgs[2].content(), "4");
    }

    #[test]
    fn strip_tool_calls() {
        let mut msgs = vec![
            Message::human("hello"),
            Message::ai_with_tool_calls(
                "",
                vec![synaptic_core::ToolCall {
                    id: "1".into(),
                    name: "test".into(),
                    arguments: serde_json::json!({}),
                }],
            ),
            Message::tool("result", "1"),
            Message::ai("final answer"),
        ];
        ContextEditingMiddleware::apply_strip_tool_calls(&mut msgs);
        assert_eq!(msgs.len(), 2);
        assert!(msgs[0].is_human());
        assert_eq!(msgs[1].content(), "final answer");
    }
}
