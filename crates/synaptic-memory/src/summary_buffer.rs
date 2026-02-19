use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use synaptic_core::{ChatModel, ChatRequest, MemoryStore, Message, SynapticError};
use tokio::sync::RwLock;

/// Hybrid memory strategy: keeps recent messages verbatim and summarizes
/// older messages when total estimated tokens exceed `max_token_limit`.
///
/// Combines the benefits of `ConversationSummaryMemory` (compact history) with
/// `ConversationTokenBufferMemory` (recent context preserved exactly).
pub struct ConversationSummaryBufferMemory {
    store: Arc<dyn MemoryStore>,
    model: Arc<dyn ChatModel>,
    summary: Arc<RwLock<HashMap<String, String>>>,
    max_token_limit: usize,
}

impl ConversationSummaryBufferMemory {
    /// Create a new summary buffer memory.
    ///
    /// - `store` — the underlying message store
    /// - `model` — the ChatModel used to generate summaries
    /// - `max_token_limit` — when total estimated tokens exceed this, older messages are summarized
    pub fn new(
        store: Arc<dyn MemoryStore>,
        model: Arc<dyn ChatModel>,
        max_token_limit: usize,
    ) -> Self {
        Self {
            store,
            model,
            summary: Arc::new(RwLock::new(HashMap::new())),
            max_token_limit,
        }
    }

    fn estimate_tokens(text: &str) -> usize {
        (text.len() / 4).max(1)
    }

    async fn summarize(&self, messages: &[Message]) -> Result<String, SynapticError> {
        let conversation = messages
            .iter()
            .map(|m| format!("{}: {}", m.role(), m.content()))
            .collect::<Vec<_>>()
            .join("\n");

        let prompt = format!("Summarize the following conversation concisely:\n\n{conversation}");
        let request = ChatRequest::new(vec![Message::human(prompt)]);
        let response = self.model.chat(request).await?;
        Ok(response.message.content().to_string())
    }
}

#[async_trait]
impl MemoryStore for ConversationSummaryBufferMemory {
    async fn append(&self, session_id: &str, message: Message) -> Result<(), SynapticError> {
        self.store.append(session_id, message).await?;

        let messages = self.store.load(session_id).await?;
        let total_tokens: usize = messages
            .iter()
            .map(|m| Self::estimate_tokens(m.content()))
            .sum();

        if total_tokens > self.max_token_limit && messages.len() > 1 {
            // Find split point: keep as many recent messages as fit within half the limit
            let half_limit = self.max_token_limit / 2;
            let mut recent_tokens = 0;
            let mut split_point = messages.len();

            for (i, msg) in messages.iter().enumerate().rev() {
                let tokens = Self::estimate_tokens(msg.content());
                if recent_tokens + tokens > half_limit {
                    split_point = i + 1;
                    break;
                }
                recent_tokens += tokens;
            }

            // Ensure we summarize at least something
            if split_point == 0 {
                split_point = 1;
            }
            if split_point >= messages.len() {
                split_point = messages.len() - 1;
            }

            let older = &messages[..split_point];
            let recent = &messages[split_point..];

            // Build summary incorporating existing summary
            let existing_summary = {
                let summaries = self.summary.read().await;
                summaries.get(session_id).cloned()
            };

            let to_summarize = if let Some(ref existing) = existing_summary {
                let mut with_context =
                    vec![Message::system(format!("Previous summary: {existing}"))];
                with_context.extend_from_slice(older);
                with_context
            } else {
                older.to_vec()
            };

            let new_summary = self.summarize(&to_summarize).await?;

            {
                let mut summaries = self.summary.write().await;
                summaries.insert(session_id.to_string(), new_summary);
            }

            // Replace store contents with just the recent messages
            self.store.clear(session_id).await?;
            for msg in recent {
                self.store.append(session_id, msg.clone()).await?;
            }
        }

        Ok(())
    }

    async fn load(&self, session_id: &str) -> Result<Vec<Message>, SynapticError> {
        let messages = self.store.load(session_id).await?;
        let summaries = self.summary.read().await;

        if let Some(summary_text) = summaries.get(session_id) {
            let mut result = vec![Message::system(format!(
                "Summary of earlier conversation: {summary_text}"
            ))];
            result.extend(messages);
            Ok(result)
        } else {
            Ok(messages)
        }
    }

    async fn clear(&self, session_id: &str) -> Result<(), SynapticError> {
        self.store.clear(session_id).await?;
        let mut summaries = self.summary.write().await;
        summaries.remove(session_id);
        Ok(())
    }
}
