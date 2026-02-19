use async_trait::async_trait;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use synaptic_core::{ChatModel, ChatRequest, Message, SynapticError};
use synaptic_middleware::{AgentMiddleware, ModelRequest};

use crate::backend::Backend;

/// Middleware that auto-summarizes conversation history when approaching the token limit.
///
/// Before each model call, estimates the token count. If it exceeds
/// `max_input_tokens * threshold_fraction`, older messages are summarized by the model
/// and the full history is offloaded to a file in the backend.
pub struct DeepSummarizationMiddleware {
    backend: Arc<dyn Backend>,
    model: Arc<dyn ChatModel>,
    max_input_tokens: usize,
    threshold_fraction: f64,
    file_counter: AtomicUsize,
}

impl DeepSummarizationMiddleware {
    pub fn new(
        backend: Arc<dyn Backend>,
        model: Arc<dyn ChatModel>,
        max_input_tokens: usize,
        threshold_fraction: f64,
    ) -> Self {
        Self {
            backend,
            model,
            max_input_tokens,
            threshold_fraction,
            file_counter: AtomicUsize::new(0),
        }
    }

    fn estimate_tokens(messages: &[Message]) -> usize {
        // ~4 chars per token heuristic
        messages.iter().map(|m| m.content().len() / 4 + 1).sum()
    }
}

#[async_trait]
impl AgentMiddleware for DeepSummarizationMiddleware {
    async fn before_model(&self, request: &mut ModelRequest) -> Result<(), SynapticError> {
        let threshold = (self.max_input_tokens as f64 * self.threshold_fraction) as usize;
        let estimated = Self::estimate_tokens(&request.messages);

        if estimated <= threshold || request.messages.len() <= 2 {
            return Ok(());
        }

        // Save full history to backend
        let counter = self.file_counter.fetch_add(1, Ordering::Relaxed);
        let history_path = format!(".context/history_{}.md", counter);
        let full_history = request
            .messages
            .iter()
            .map(|m| format!("## {}\n{}", m.role(), m.content()))
            .collect::<Vec<_>>()
            .join("\n\n");
        let _ = self.backend.write_file(&history_path, &full_history).await;

        // Keep last 2 messages, summarize the rest
        let keep_count = 2.min(request.messages.len());
        let to_summarize = &request.messages[..request.messages.len() - keep_count];

        if to_summarize.is_empty() {
            return Ok(());
        }

        let summary_prompt = format!(
            "Summarize the following conversation concisely, \
             preserving key decisions, facts, and context:\n\n{}",
            to_summarize
                .iter()
                .map(|m| format!("{}: {}", m.role(), m.content()))
                .collect::<Vec<_>>()
                .join("\n")
        );

        let summary_request = ChatRequest::new(vec![Message::human(summary_prompt)]);
        let summary_response = self.model.chat(summary_request).await?;
        let summary = summary_response.message.content().to_string();

        // Replace old messages with summary + recent messages
        let recent: Vec<Message> =
            request.messages[request.messages.len() - keep_count..].to_vec();
        request.messages = vec![Message::system(format!(
            "[Conversation summary (full history saved to {})]\n{}",
            history_path, summary
        ))];
        request.messages.extend(recent);

        Ok(())
    }
}
