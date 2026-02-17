use std::collections::HashMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use synaptic_core::SynapseError;
use tokio::sync::RwLock;

/// Configuration identifying a checkpoint (thread/conversation).
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct CheckpointConfig {
    pub thread_id: String,
}

impl CheckpointConfig {
    pub fn new(thread_id: impl Into<String>) -> Self {
        Self {
            thread_id: thread_id.into(),
        }
    }
}

/// A snapshot of graph state at a point in execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    pub state: serde_json::Value,
    pub next_node: Option<String>,
}

/// Trait for persisting graph state checkpoints.
#[async_trait]
pub trait Checkpointer: Send + Sync {
    async fn put(
        &self,
        config: &CheckpointConfig,
        checkpoint: &Checkpoint,
    ) -> Result<(), SynapseError>;
    async fn get(&self, config: &CheckpointConfig) -> Result<Option<Checkpoint>, SynapseError>;
    async fn list(&self, config: &CheckpointConfig) -> Result<Vec<Checkpoint>, SynapseError>;
}

/// In-memory checkpointer (for development/testing).
#[derive(Default)]
pub struct MemorySaver {
    store: RwLock<HashMap<String, Vec<Checkpoint>>>,
}

impl MemorySaver {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl Checkpointer for MemorySaver {
    async fn put(
        &self,
        config: &CheckpointConfig,
        checkpoint: &Checkpoint,
    ) -> Result<(), SynapseError> {
        let mut store = self.store.write().await;
        store
            .entry(config.thread_id.clone())
            .or_default()
            .push(checkpoint.clone());
        Ok(())
    }

    async fn get(&self, config: &CheckpointConfig) -> Result<Option<Checkpoint>, SynapseError> {
        let store = self.store.read().await;
        Ok(store.get(&config.thread_id).and_then(|v| v.last().cloned()))
    }

    async fn list(&self, config: &CheckpointConfig) -> Result<Vec<Checkpoint>, SynapseError> {
        let store = self.store.read().await;
        Ok(store.get(&config.thread_id).cloned().unwrap_or_default())
    }
}
