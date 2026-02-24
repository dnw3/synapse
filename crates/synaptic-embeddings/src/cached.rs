use std::sync::Arc;

use async_trait::async_trait;
use sha2::{Digest, Sha256};
use synaptic_core::{Store, SynapticError};

use crate::Embeddings;

/// An embeddings wrapper that caches results in a [`Store`] backend.
///
/// Previously computed embeddings are stored in the provided [`Store`] keyed
/// by the SHA-256 hash of the input text. On subsequent calls, cached
/// embeddings are returned directly, and only uncached texts are sent to the
/// inner embeddings provider.
///
/// This aligns with LangChain Python's `CacheBackedEmbeddings` architecture,
/// allowing any `Store` implementation (in-memory, SQLite, PostgreSQL, Redis,
/// etc.) to serve as the caching backend.
pub struct CacheBackedEmbeddings {
    inner: Arc<dyn Embeddings>,
    store: Arc<dyn Store>,
    namespace: String,
}

impl CacheBackedEmbeddings {
    /// Create a new cached embeddings wrapper.
    ///
    /// - `inner` — the underlying embeddings provider to delegate to on cache misses.
    /// - `store` — the [`Store`] backend for persisting cached embeddings.
    /// - `namespace` — a logical namespace within the store (combined with
    ///   `"embedding_cache"` as the prefix).
    pub fn new(
        inner: Arc<dyn Embeddings>,
        store: Arc<dyn Store>,
        namespace: impl Into<String>,
    ) -> Self {
        Self {
            inner,
            store,
            namespace: namespace.into(),
        }
    }

    /// Build the store namespace for this cache instance.
    fn store_namespace(&self) -> Vec<String> {
        vec!["embedding_cache".to_string(), self.namespace.clone()]
    }

    /// Compute the SHA-256 hash of a text, returned as a hex string.
    fn hash_key(text: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(text.as_bytes());
        format!("{:x}", hasher.finalize())
    }
}

#[async_trait]
impl Embeddings for CacheBackedEmbeddings {
    async fn embed_documents(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, SynapticError> {
        let ns = self.store_namespace();
        let ns_refs: Vec<&str> = ns.iter().map(|s| s.as_str()).collect();

        // Check cache for each text
        let mut results: Vec<Option<Vec<f32>>> = Vec::with_capacity(texts.len());
        let mut uncached_indices: Vec<usize> = Vec::new();
        let mut uncached_texts: Vec<&str> = Vec::new();

        for (i, text) in texts.iter().enumerate() {
            let key = Self::hash_key(text);
            if let Some(item) = self.store.get(&ns_refs, &key).await? {
                // Deserialize the cached embedding
                let embedding: Vec<f32> = serde_json::from_value(item.value)
                    .map_err(|e| SynapticError::Store(format!("cache deserialize error: {e}")))?;
                results.push(Some(embedding));
            } else {
                results.push(None);
                uncached_indices.push(i);
                uncached_texts.push(text);
            }
        }

        // Embed uncached texts
        if !uncached_texts.is_empty() {
            let new_embeddings = self.inner.embed_documents(&uncached_texts).await?;

            // Store new embeddings in cache
            for (idx, embedding) in uncached_indices.iter().zip(new_embeddings.into_iter()) {
                let key = Self::hash_key(texts[*idx]);
                let value = serde_json::to_value(&embedding)
                    .map_err(|e| SynapticError::Store(format!("cache serialize error: {e}")))?;
                self.store.put(&ns_refs, &key, value).await?;
                results[*idx] = Some(embedding);
            }
        }

        // All results should now be Some
        Ok(results.into_iter().map(|r| r.unwrap()).collect())
    }

    async fn embed_query(&self, text: &str) -> Result<Vec<f32>, SynapticError> {
        let ns = self.store_namespace();
        let ns_refs: Vec<&str> = ns.iter().map(|s| s.as_str()).collect();
        let key = Self::hash_key(text);

        // Check cache
        if let Some(item) = self.store.get(&ns_refs, &key).await? {
            let embedding: Vec<f32> = serde_json::from_value(item.value)
                .map_err(|e| SynapticError::Store(format!("cache deserialize error: {e}")))?;
            return Ok(embedding);
        }

        // Cache miss: compute embedding
        let embedding = self.inner.embed_query(text).await?;

        // Store in cache
        let value = serde_json::to_value(&embedding)
            .map_err(|e| SynapticError::Store(format!("cache serialize error: {e}")))?;
        self.store.put(&ns_refs, &key, value).await?;

        Ok(embedding)
    }
}
