use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use synaptic_core::{ChatModel, ChatRequest, MemoryStore, Message, SynapseError};
use tokio::sync::RwLock;

/// A memory strategy that summarizes older messages using a ChatModel.
///
/// Keeps the most recent `buffer_size` messages verbatim. When the total
/// number of stored messages exceeds `buffer_size * 2`, the older messages
/// are summarized into a single system message that is prepended on `load`.
pub struct ConversationSummaryMemory {
    store: Arc<dyn MemoryStore>,
    model: Arc<dyn ChatModel>,
    summary: Arc<RwLock<HashMap<String, String>>>,
    buffer_size: usize,
}

impl ConversationSummaryMemory {
    /// Create a new summary memory.
    ///
    /// - `store` — the underlying message store
    /// - `model` — the ChatModel used to generate summaries
    /// - `buffer_size` — number of recent messages to keep verbatim
    pub fn new(store: Arc<dyn MemoryStore>, model: Arc<dyn ChatModel>, buffer_size: usize) -> Self {
        Self {
            store,
            model,
            summary: Arc::new(RwLock::new(HashMap::new())),
            buffer_size,
        }
    }

    /// Generate a summary of the given messages using the ChatModel.
    async fn summarize(&self, messages: &[Message]) -> Result<String, SynapseError> {
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
impl MemoryStore for ConversationSummaryMemory {
    async fn append(&self, session_id: &str, message: Message) -> Result<(), SynapseError> {
        self.store.append(session_id, message).await?;

        // Check if we need to trigger summarization
        let messages = self.store.load(session_id).await?;
        if messages.len() > self.buffer_size * 2 {
            // Summarize the older messages (everything except the most recent buffer_size)
            let split_point = messages.len() - self.buffer_size;
            let older = &messages[..split_point];
            let recent = &messages[split_point..];

            // Build new summary incorporating any existing summary
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

            // Update the summary
            {
                let mut summaries = self.summary.write().await;
                summaries.insert(session_id.to_string(), new_summary);
            }

            // Replace the store contents with just the recent messages
            self.store.clear(session_id).await?;
            for msg in recent {
                self.store.append(session_id, msg.clone()).await?;
            }
        }

        Ok(())
    }

    async fn load(&self, session_id: &str) -> Result<Vec<Message>, SynapseError> {
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

    async fn clear(&self, session_id: &str) -> Result<(), SynapseError> {
        self.store.clear(session_id).await?;
        let mut summaries = self.summary.write().await;
        summaries.remove(session_id);
        Ok(())
    }
}
