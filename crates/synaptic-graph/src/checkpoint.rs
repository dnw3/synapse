use std::collections::HashMap;

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use synaptic_core::SynapticError;

/// Configuration identifying a checkpoint (thread/conversation).
#[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
pub struct CheckpointConfig {
    pub thread_id: String,
    /// Optional: target a specific checkpoint for time-travel.
    /// When `None`, operations target the latest checkpoint.
    pub checkpoint_id: Option<String>,
}

impl CheckpointConfig {
    pub fn new(thread_id: impl Into<String>) -> Self {
        Self {
            thread_id: thread_id.into(),
            checkpoint_id: None,
        }
    }

    /// Create a config targeting a specific checkpoint (for time-travel).
    pub fn with_checkpoint_id(
        thread_id: impl Into<String>,
        checkpoint_id: impl Into<String>,
    ) -> Self {
        Self {
            thread_id: thread_id.into(),
            checkpoint_id: Some(checkpoint_id.into()),
        }
    }
}

/// A snapshot of graph state at a point in execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Unique identifier for this checkpoint.
    pub id: String,
    /// Serialized graph state.
    pub state: serde_json::Value,
    /// The next node to execute (or None if graph completed).
    pub next_node: Option<String>,
    /// ID of the previous checkpoint (for traversing history).
    pub parent_id: Option<String>,
    /// Metadata about this checkpoint (node name, timestamp, etc.).
    pub metadata: HashMap<String, serde_json::Value>,
}

impl Checkpoint {
    /// Create a new checkpoint with auto-generated ID.
    pub fn new(state: serde_json::Value, next_node: Option<String>) -> Self {
        Self {
            id: generate_checkpoint_id(),
            state,
            next_node,
            parent_id: None,
            metadata: HashMap::new(),
        }
    }

    /// Set the parent checkpoint ID.
    pub fn with_parent(mut self, parent_id: impl Into<String>) -> Self {
        self.parent_id = Some(parent_id.into());
        self
    }

    /// Add metadata to the checkpoint.
    pub fn with_metadata(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.metadata.insert(key.into(), value);
        self
    }
}

fn generate_checkpoint_id() -> String {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::{SystemTime, UNIX_EPOCH};

    static COUNTER: AtomicU64 = AtomicU64::new(0);

    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{ts:x}-{seq:04x}")
}

/// Trait for persisting graph state checkpoints.
#[async_trait]
pub trait Checkpointer: Send + Sync {
    /// Save a checkpoint for the given thread.
    async fn put(
        &self,
        config: &CheckpointConfig,
        checkpoint: &Checkpoint,
    ) -> Result<(), SynapticError>;

    /// Get a checkpoint. If `config.checkpoint_id` is set, returns that specific
    /// checkpoint; otherwise returns the latest checkpoint for the thread.
    async fn get(&self, config: &CheckpointConfig) -> Result<Option<Checkpoint>, SynapticError>;

    /// List all checkpoints for a thread, ordered oldest to newest.
    async fn list(&self, config: &CheckpointConfig) -> Result<Vec<Checkpoint>, SynapticError>;
}
