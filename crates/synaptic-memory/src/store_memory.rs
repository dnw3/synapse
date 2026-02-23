use std::sync::Arc;

use async_trait::async_trait;
use synaptic_core::{MemoryStore, Message, Store, SynapticError};

/// `MemoryStore` implementation backed by any [`Store`].
///
/// Messages are stored under namespace `["memory", "{session_id}"]` with key `"messages"`.
/// Summaries (used by summary strategies) are stored under the same namespace with key `"summary"`.
///
/// This replaces the old `InMemoryStore` (in-memory only) and `FileChatMessageHistory`
/// (file-only) with a single implementation that works with *any* Store backend
/// (InMemoryStore, FileStore, RedisStore, etc.).
pub struct ChatMessageHistory {
    store: Arc<dyn Store>,
}

impl ChatMessageHistory {
    /// Create a new `ChatMessageHistory` backed by the given store.
    pub fn new(store: Arc<dyn Store>) -> Self {
        Self { store }
    }

    /// Get the summary for a session (used by summary memory strategies).
    pub async fn get_summary(&self, session_id: &str) -> Result<Option<String>, SynapticError> {
        let item = self.store.get(&["memory", session_id], "summary").await?;
        Ok(item.and_then(|i| i.value.as_str().map(String::from)))
    }

    /// Set the summary for a session (used by summary memory strategies).
    pub async fn set_summary(&self, session_id: &str, summary: &str) -> Result<(), SynapticError> {
        self.store
            .put(
                &["memory", session_id],
                "summary",
                serde_json::Value::String(summary.to_string()),
            )
            .await
    }

    /// Return a reference to the underlying store.
    pub fn store(&self) -> &Arc<dyn Store> {
        &self.store
    }
}

#[async_trait]
impl MemoryStore for ChatMessageHistory {
    async fn append(&self, session_id: &str, message: Message) -> Result<(), SynapticError> {
        let mut messages = self.load(session_id).await?;
        messages.push(message);
        let value = serde_json::to_value(&messages)
            .map_err(|e| SynapticError::Memory(format!("failed to serialize messages: {e}")))?;
        self.store
            .put(&["memory", session_id], "messages", value)
            .await
    }

    async fn load(&self, session_id: &str) -> Result<Vec<Message>, SynapticError> {
        let item = self.store.get(&["memory", session_id], "messages").await?;
        match item {
            Some(item) => {
                let messages: Vec<Message> = serde_json::from_value(item.value).map_err(|e| {
                    SynapticError::Memory(format!("failed to deserialize messages: {e}"))
                })?;
                Ok(messages)
            }
            None => Ok(Vec::new()),
        }
    }

    async fn clear(&self, session_id: &str) -> Result<(), SynapticError> {
        self.store
            .delete(&["memory", session_id], "messages")
            .await?;
        self.store
            .delete(&["memory", session_id], "summary")
            .await?;
        Ok(())
    }
}
