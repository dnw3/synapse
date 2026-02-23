use std::sync::Arc;

use async_trait::async_trait;
use synaptic_core::{Store, SynapticError};

use crate::checkpoint::{Checkpoint, CheckpointConfig, Checkpointer};

/// `Checkpointer` implementation backed by any [`Store`].
///
/// Checkpoints are stored under namespace `["checkpoints", "{thread_id}"]`
/// with the checkpoint ID as the key.
///
/// This replaces `MemorySaver` (in-memory only) and `FileSaver` (file-only)
/// with a single implementation that works with any Store backend.
pub struct StoreCheckpointer {
    store: Arc<dyn Store>,
}

impl StoreCheckpointer {
    /// Create a new checkpointer backed by the given store.
    pub fn new(store: Arc<dyn Store>) -> Self {
        Self { store }
    }
}

#[async_trait]
impl Checkpointer for StoreCheckpointer {
    async fn put(
        &self,
        config: &CheckpointConfig,
        checkpoint: &Checkpoint,
    ) -> Result<(), SynapticError> {
        let value = serde_json::to_value(checkpoint)
            .map_err(|e| SynapticError::Graph(format!("failed to serialize checkpoint: {e}")))?;
        self.store
            .put(&["checkpoints", &config.thread_id], &checkpoint.id, value)
            .await
    }

    async fn get(&self, config: &CheckpointConfig) -> Result<Option<Checkpoint>, SynapticError> {
        // If a specific checkpoint_id is requested, fetch it directly
        if let Some(ref target_id) = config.checkpoint_id {
            let item = self
                .store
                .get(&["checkpoints", &config.thread_id], target_id)
                .await?;
            return match item {
                Some(item) => {
                    let checkpoint: Checkpoint =
                        serde_json::from_value(item.value).map_err(|e| {
                            SynapticError::Graph(format!("failed to deserialize checkpoint: {e}"))
                        })?;
                    Ok(Some(checkpoint))
                }
                None => Ok(None),
            };
        }

        // Otherwise return the latest — search all, sort by ID (timestamp-hex), take last
        let items = self
            .store
            .search(&["checkpoints", &config.thread_id], None, 10_000)
            .await?;

        if items.is_empty() {
            return Ok(None);
        }

        // IDs are timestamp-hex format, alphabetical = chronological
        let latest = items.into_iter().max_by(|a, b| a.key.cmp(&b.key)).unwrap();

        let checkpoint: Checkpoint = serde_json::from_value(latest.value)
            .map_err(|e| SynapticError::Graph(format!("failed to deserialize checkpoint: {e}")))?;
        Ok(Some(checkpoint))
    }

    async fn list(&self, config: &CheckpointConfig) -> Result<Vec<Checkpoint>, SynapticError> {
        let items = self
            .store
            .search(&["checkpoints", &config.thread_id], None, 10_000)
            .await?;

        let mut checkpoints: Vec<Checkpoint> = items
            .into_iter()
            .map(|item| {
                serde_json::from_value(item.value).map_err(|e| {
                    SynapticError::Graph(format!("failed to deserialize checkpoint: {e}"))
                })
            })
            .collect::<Result<Vec<_>, _>>()?;

        // Sort by ID (oldest first) — IDs are timestamp-hex, alphabetical = chronological
        checkpoints.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(checkpoints)
    }
}
