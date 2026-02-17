use std::sync::Arc;

use async_trait::async_trait;
use synaptic_core::{MemoryStore, Message, SynapseError};

/// A memory strategy that keeps only the last `window_size` messages.
///
/// All messages are stored in the underlying store, but `load` returns
/// only the most recent `window_size` messages.
pub struct ConversationWindowMemory {
    store: Arc<dyn MemoryStore>,
    window_size: usize,
}

impl ConversationWindowMemory {
    /// Create a new window memory wrapping the given store.
    ///
    /// `window_size` is the maximum number of messages returned by `load`.
    pub fn new(store: Arc<dyn MemoryStore>, window_size: usize) -> Self {
        Self { store, window_size }
    }
}

#[async_trait]
impl MemoryStore for ConversationWindowMemory {
    async fn append(&self, session_id: &str, message: Message) -> Result<(), SynapseError> {
        self.store.append(session_id, message).await
    }

    async fn load(&self, session_id: &str) -> Result<Vec<Message>, SynapseError> {
        let messages = self.store.load(session_id).await?;
        if messages.len() <= self.window_size {
            Ok(messages)
        } else {
            let start = messages.len() - self.window_size;
            Ok(messages[start..].to_vec())
        }
    }

    async fn clear(&self, session_id: &str) -> Result<(), SynapseError> {
        self.store.clear(session_id).await
    }
}
