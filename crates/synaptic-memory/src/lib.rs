mod buffer;
mod file_history;
mod history;
mod summary;
mod summary_buffer;
mod token_buffer;
mod window;

pub use buffer::ConversationBufferMemory;
pub use file_history::FileChatMessageHistory;
pub use history::RunnableWithMessageHistory;
pub use summary::ConversationSummaryMemory;
pub use summary_buffer::ConversationSummaryBufferMemory;
pub use token_buffer::ConversationTokenBufferMemory;
pub use window::ConversationWindowMemory;

use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use synaptic_core::{MemoryStore, Message, SynapseError};
use tokio::sync::RwLock;

/// In-memory implementation of `MemoryStore`, storing messages keyed by session ID.
#[derive(Default, Clone)]
pub struct InMemoryStore {
    sessions: Arc<RwLock<HashMap<String, Vec<Message>>>>,
}

impl InMemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl MemoryStore for InMemoryStore {
    async fn append(&self, session_id: &str, message: Message) -> Result<(), SynapseError> {
        let mut sessions = self.sessions.write().await;
        let session = sessions.entry(session_id.to_string()).or_default();
        session.push(message);
        Ok(())
    }

    async fn load(&self, session_id: &str) -> Result<Vec<Message>, SynapseError> {
        let sessions = self.sessions.read().await;
        Ok(sessions.get(session_id).cloned().unwrap_or_default())
    }

    async fn clear(&self, session_id: &str) -> Result<(), SynapseError> {
        let mut sessions = self.sessions.write().await;
        sessions.remove(session_id);
        Ok(())
    }
}
