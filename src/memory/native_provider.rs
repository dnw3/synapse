use std::sync::Arc;

use async_trait::async_trait;
use synaptic::core::SynapticError;
use synaptic::memory::{CommitResult, MemoryProvider, MemoryResult};

use crate::memory::LongTermMemory;

/// A [`MemoryProvider`] implementation backed by Synapse's native
/// [`LongTermMemory`] store.
///
/// This bridges the generic `MemoryProvider` trait (defined in synaptic-memory)
/// to the existing LTM infrastructure so that framework-level consumers can use
/// Synapse's local memory without knowing about the implementation details.
#[allow(dead_code)]
pub struct NativeMemoryProvider {
    ltm: Arc<LongTermMemory>,
}

#[allow(dead_code)]
impl NativeMemoryProvider {
    pub fn new(ltm: Arc<LongTermMemory>) -> Self {
        Self { ltm }
    }
}

#[async_trait]
impl MemoryProvider for NativeMemoryProvider {
    /// Native LTM does not maintain a separate short-term message buffer, so
    /// this is a no-op.
    async fn add_message(
        &self,
        _session_key: &str,
        _role: &str,
        _content: &str,
    ) -> Result<(), SynapticError> {
        Ok(())
    }

    /// Native LTM does not track context/skill usage statistics, so this is a
    /// no-op.
    async fn record_usage(
        &self,
        _session_key: &str,
        _context_uris: &[String],
        _skill_uris: &[String],
    ) -> Result<(), SynapticError> {
        Ok(())
    }

    /// Retrieve the most relevant memories for `query` by delegating to the
    /// LTM hybrid-search / keyword-search pipeline.
    async fn recall(&self, query: &str, limit: usize) -> Result<Vec<MemoryResult>, SynapticError> {
        let contents = self.ltm.recall(query, limit).await;
        let results = contents
            .into_iter()
            .enumerate()
            .map(|(i, content)| MemoryResult {
                uri: format!("ltm:{}", i),
                content,
                score: 1.0,
                category: None,
                layer: Some("semantic".to_string()),
                metadata: serde_json::Value::Null,
            })
            .collect();
        Ok(results)
    }

    /// Search memories, optionally scoped to a session.  The native LTM does
    /// not distinguish sessions, so `session_key` is ignored.
    async fn search(
        &self,
        query: &str,
        _session_key: Option<&str>,
        limit: usize,
    ) -> Result<Vec<MemoryResult>, SynapticError> {
        self.recall(query, limit).await
    }

    /// Commit is a no-op for the native provider — Synapse manages its own
    /// flush lifecycle (e.g. `flush_before_compact`).
    async fn commit(&self, _session_key: &str) -> Result<CommitResult, SynapticError> {
        Ok(CommitResult::default())
    }

    /// The native LTM does not support arbitrary URI-based resource ingestion.
    async fn add_resource(&self, _uri: &str) -> Result<(), SynapticError> {
        Err(SynapticError::Tool(
            "NativeMemoryProvider does not support resource ingestion".into(),
        ))
    }

    /// The native LTM does not maintain per-user profiles.
    async fn get_profile(&self, _user_id: &str) -> Result<Option<String>, SynapticError> {
        Ok(None)
    }
}
