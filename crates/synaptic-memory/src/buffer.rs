use std::sync::Arc;

use async_trait::async_trait;
use synaptic_core::{MemoryStore, Message, SynapticError};

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
    async fn append(&self, session_id: &str, message: Message) -> Result<(), SynapticError> {
        self.store.append(session_id, message).await
    }

    async fn load(&self, session_id: &str) -> Result<Vec<Message>, SynapticError> {
        self.store.load(session_id).await
    }

    async fn clear(&self, session_id: &str) -> Result<(), SynapticError> {
        self.store.clear(session_id).await
    }
}
