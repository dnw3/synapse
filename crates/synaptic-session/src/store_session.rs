use std::sync::Arc;

use serde::{Deserialize, Serialize};
use synaptic_core::{now_iso, Store, SynapticError};
use synaptic_graph::StoreCheckpointer;
use synaptic_memory::ChatMessageHistory;

/// Metadata about a session.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id: String,
    pub created_at: String,
}

/// Store-backed session manager.
///
/// Session metadata is stored under namespace `["sessions"]`, key = session_id.
/// Messages are accessed through [`ChatMessageHistory`] (same store).
/// Checkpoints are accessed through [`StoreCheckpointer`] (same store).
pub struct SessionManager {
    store: Arc<dyn Store>,
}

impl SessionManager {
    /// Create a new session manager backed by the given store.
    pub fn new(store: Arc<dyn Store>) -> Self {
        Self { store }
    }

    /// Create a new session with a unique ID.
    pub async fn create_session(&self) -> Result<String, SynapticError> {
        let id = uuid::Uuid::new_v4().to_string();
        let info = SessionInfo {
            id: id.clone(),
            created_at: now_iso(),
        };
        let value = serde_json::to_value(&info)
            .map_err(|e| SynapticError::Store(format!("failed to serialize session info: {e}")))?;
        self.store.put(&["sessions"], &id, value).await?;
        Ok(id)
    }

    /// List all sessions.
    pub async fn list_sessions(&self) -> Result<Vec<SessionInfo>, SynapticError> {
        let items = self.store.search(&["sessions"], None, 10_000).await?;
        let mut sessions: Vec<SessionInfo> = items
            .into_iter()
            .filter_map(|item| serde_json::from_value(item.value).ok())
            .collect();
        sessions.sort_by(|a, b| a.created_at.cmp(&b.created_at));
        Ok(sessions)
    }

    /// Get session info by ID.
    pub async fn get_session(&self, id: &str) -> Result<Option<SessionInfo>, SynapticError> {
        let item = self.store.get(&["sessions"], id).await?;
        match item {
            Some(item) => {
                let info: SessionInfo = serde_json::from_value(item.value).map_err(|e| {
                    SynapticError::Store(format!("failed to deserialize session info: {e}"))
                })?;
                Ok(Some(info))
            }
            None => Ok(None),
        }
    }

    /// Delete a session and all its associated data (messages, summaries, checkpoints).
    pub async fn delete_session(&self, id: &str) -> Result<(), SynapticError> {
        // Delete session metadata
        self.store.delete(&["sessions"], id).await?;

        // Delete messages and summary
        self.store.delete(&["memory", id], "messages").await?;
        self.store.delete(&["memory", id], "summary").await?;

        // Delete checkpoints — search and delete each one
        let checkpoints = self
            .store
            .search(&["checkpoints", id], None, 10_000)
            .await?;
        for ckpt in checkpoints {
            self.store.delete(&["checkpoints", id], &ckpt.key).await?;
        }

        Ok(())
    }

    /// Get a `ChatMessageHistory` that shares the same store.
    pub fn memory(&self) -> ChatMessageHistory {
        ChatMessageHistory::new(self.store.clone())
    }

    /// Get a `StoreCheckpointer` that shares the same store.
    pub fn checkpointer(&self) -> StoreCheckpointer {
        StoreCheckpointer::new(self.store.clone())
    }

    /// Get a reference to the underlying store.
    pub fn store(&self) -> &Arc<dyn Store> {
        &self.store
    }
}
