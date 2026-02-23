use std::sync::Arc;

use crate::Condenser;
use async_trait::async_trait;
use synaptic_core::{ChatModel, ChatRequest, Message, SynapticError};

/// Summarizes older messages using an LLM, keeping recent messages intact.
pub struct LlmSummarizingCondenser {
    model: Arc<dyn ChatModel>,
    max_tokens: usize,
    keep_recent: usize,
}

impl LlmSummarizingCondenser {
    pub fn new(model: Arc<dyn ChatModel>, max_tokens: usize, keep_recent: usize) -> Self {
        Self {
            model,
            max_tokens,
            keep_recent,
        }
    }
}

#[async_trait]
impl Condenser for LlmSummarizingCondenser {
    async fn condense(&self, messages: Vec<Message>) -> Result<Vec<Message>, SynapticError> {
        // If messages are within budget, return as-is
        let estimated_tokens: usize = messages.iter().map(|m| m.content().len() / 4 + 4).sum();
        if estimated_tokens <= self.max_tokens {
            return Ok(messages);
        }

        // Split into system (if any), old messages to summarize, and recent to keep
        let (system_msg, rest) = if !messages.is_empty() && messages[0].is_system() {
            (Some(messages[0].clone()), &messages[1..])
        } else {
            (None, messages.as_slice())
        };

        if rest.len() <= self.keep_recent {
            return Ok(messages);
        }

        let split = rest.len() - self.keep_recent;
        let to_summarize = &rest[..split];
        let to_keep = &rest[split..];

        // Build summarization prompt
        let mut summary_text = String::new();
        for msg in to_summarize {
            summary_text.push_str(&format!("{}: {}\n", msg.role(), msg.content()));
        }

        let prompt = format!(
            "Summarize the following conversation concisely, preserving key information:\n\n{}",
            summary_text
        );

        let request = ChatRequest::new(vec![Message::human(prompt)]);
        let response = self.model.chat(request).await?;
        let summary = response.message.content().to_string();

        // Reassemble: system + summary + recent
        let mut result = Vec::new();
        if let Some(sys) = system_msg {
            result.push(sys);
        }
        result.push(Message::system(format!(
            "[Conversation Summary]\n{}",
            summary
        )));
        result.extend_from_slice(to_keep);

        Ok(result)
    }
}
