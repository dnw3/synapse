use std::sync::Arc;

use async_trait::async_trait;
use synaptic_core::{MemoryStore, Message, SynapseError};

/// A memory strategy that stores the full conversation buffer.
///
/// This is a passthrough wrapper around any `MemoryStore` that makes
/// the "keep everything" strategy explicit and composable.
pub struct ConversationBufferMemory {
    store: Arc<dyn MemoryStore>,
}

impl ConversationBufferMemory {
    /// Create a new buffer memory wrapping the given store.
    pub fn new(store: Arc<dyn MemoryStore>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl MemoryStore for ConversationBufferMemory {
    async fn append(&self, session_id: &str, message: Message) -> Result<(), SynapseError> {
        self.store.append(session_id, message).await
    }

    async fn load(&self, session_id: &str) -> Result<Vec<Message>, SynapseError> {
        self.store.load(session_id).await
    }

    async fn clear(&self, session_id: &str) -> Result<(), SynapseError> {
        self.store.clear(session_id).await
    }
}
