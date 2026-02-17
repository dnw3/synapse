use std::sync::Arc;

use async_trait::async_trait;
use synaptic_core::{MemoryStore, Message, SynapseError};

/// A memory strategy that keeps messages within a token budget.
///
/// Uses a simple estimator (~4 chars per token) to approximate token counts.
/// On `load`, removes the oldest messages until the total estimated tokens
/// fit within `max_tokens`.
pub struct ConversationTokenBufferMemory {
    store: Arc<dyn MemoryStore>,
    max_tokens: usize,
}

impl ConversationTokenBufferMemory {
    /// Create a new token buffer memory wrapping the given store.
    pub fn new(store: Arc<dyn MemoryStore>, max_tokens: usize) -> Self {
        Self { store, max_tokens }
    }

    /// Estimate the number of tokens in a text string.
    ///
    /// Uses the simple heuristic of ~4 characters per token, with a minimum of 1.
    pub fn estimate_tokens(text: &str) -> usize {
        text.len() / 4 + 1
    }
}

#[async_trait]
impl MemoryStore for ConversationTokenBufferMemory {
    async fn append(&self, session_id: &str, message: Message) -> Result<(), SynapseError> {
        self.store.append(session_id, message).await
    }

    async fn load(&self, session_id: &str) -> Result<Vec<Message>, SynapseError> {
        let messages = self.store.load(session_id).await?;

        // Calculate total tokens for all messages
        let total_tokens: usize = messages
            .iter()
            .map(|m| Self::estimate_tokens(m.content()))
            .sum();

        if total_tokens <= self.max_tokens {
            return Ok(messages);
        }

        // Remove oldest messages until we fit within the budget
        let mut kept = messages;
        let mut current_tokens: usize = kept
            .iter()
            .map(|m| Self::estimate_tokens(m.content()))
            .sum();

        while current_tokens > self.max_tokens && !kept.is_empty() {
            let removed = kept.remove(0);
            current_tokens -= Self::estimate_tokens(removed.content());
        }

        Ok(kept)
    }

    async fn clear(&self, session_id: &str) -> Result<(), SynapseError> {
        self.store.clear(session_id).await
    }
}
