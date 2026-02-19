use std::sync::Arc;

use async_trait::async_trait;
use synaptic_core::{ChatModel, ChatRequest, Message, SynapticError};

use crate::{AgentMiddleware, ModelRequest};

/// Automatically summarizes conversation history when it exceeds a token limit.
///
/// Uses a configurable token counter to estimate message sizes. When
/// the total exceeds `max_tokens`, older messages (excluding the
/// system prompt) are summarized into a single summary message using
/// the provided `ChatModel`.
pub struct SummarizationMiddleware {
    model: Arc<dyn ChatModel>,
    max_tokens: usize,
    token_counter: Box<dyn Fn(&Message) -> usize + Send + Sync>,
}

impl SummarizationMiddleware {
    /// Create a new summarization middleware.
    ///
    /// * `model` — The model to use for generating summaries.
    /// * `max_tokens` — When total tokens exceed this, summarize older messages.
    /// * `token_counter` — Function that estimates the token count for a message.
    pub fn new(
        model: Arc<dyn ChatModel>,
        max_tokens: usize,
        token_counter: impl Fn(&Message) -> usize + Send + Sync + 'static,
    ) -> Self {
        Self {
            model,
            max_tokens,
            token_counter: Box::new(token_counter),
        }
    }
}

#[async_trait]
impl AgentMiddleware for SummarizationMiddleware {
    async fn before_model(&self, request: &mut ModelRequest) -> Result<(), SynapticError> {
        let total: usize = request
            .messages
            .iter()
            .map(|m| (self.token_counter)(m))
            .sum();
        if total <= self.max_tokens {
            return Ok(());
        }

        // Keep the most recent messages that fit in half the budget,
        // summarize everything before them.
        let half_budget = self.max_tokens / 2;
        let mut keep_from = request.messages.len();
        let mut kept_tokens = 0;
        for (i, msg) in request.messages.iter().enumerate().rev() {
            let t = (self.token_counter)(msg);
            if kept_tokens + t > half_budget {
                break;
            }
            kept_tokens += t;
            keep_from = i;
        }

        if keep_from == 0 {
            // Everything fits or there's nothing to summarize
            return Ok(());
        }

        let to_summarize: Vec<_> = request.messages[..keep_from].to_vec();

        // Build a summary request
        let summary_prompt = Message::human(
            "Summarize the following conversation concisely, preserving key facts and context:\n\n"
                .to_string()
                + &to_summarize
                    .iter()
                    .map(|m| format!("{}: {}", m.role(), m.content()))
                    .collect::<Vec<_>>()
                    .join("\n"),
        );

        let summary_req = ChatRequest::new(vec![
            Message::system("You are a conversation summarizer. Output a brief summary."),
            summary_prompt,
        ]);

        let summary_resp = self.model.chat(summary_req).await?;
        let summary_text = summary_resp.message.content().to_string();

        // Replace old messages with the summary
        let mut new_messages = vec![Message::system(format!(
            "[Previous conversation summary]: {summary_text}"
        ))];
        new_messages.extend_from_slice(&request.messages[keep_from..]);
        request.messages = new_messages;

        Ok(())
    }
}
