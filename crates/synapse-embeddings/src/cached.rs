use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use synapse_core::SynapseError;
use tokio::sync::RwLock;

use crate::Embeddings;

/// An embeddings wrapper that caches results in memory.
///
/// Previously computed embeddings are stored in an in-memory hash map keyed
/// by the input text. On subsequent calls, cached embeddings are returned
/// directly, and only uncached texts are sent to the inner embeddings provider.
pub struct CacheBackedEmbeddings {
    inner: Arc<dyn Embeddings>,
    cache: Arc<RwLock<HashMap<String, Vec<f32>>>>,
}

impl CacheBackedEmbeddings {
    /// Create a new cached embeddings wrapper around the given embeddings provider.
    pub fn new(inner: Arc<dyn Embeddings>) -> Self {
        Self {
            inner,
            cache: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl Embeddings for CacheBackedEmbeddings {
    async fn embed_documents(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, SynapseError> {
        // Determine which texts need embedding
        let cache = self.cache.read().await;
        let mut results: Vec<Option<Vec<f32>>> = Vec::with_capacity(texts.len());
        let mut uncached_indices: Vec<usize> = Vec::new();
        let mut uncached_texts: Vec<&str> = Vec::new();

        for (i, text) in texts.iter().enumerate() {
            if let Some(cached) = cache.get(*text) {
                results.push(Some(cached.clone()));
            } else {
                results.push(None);
                uncached_indices.push(i);
                uncached_texts.push(text);
            }
        }
        drop(cache);

        // Embed uncached texts
        if !uncached_texts.is_empty() {
            let new_embeddings = self.inner.embed_documents(&uncached_texts).await?;

            // Store new embeddings in cache
            let mut cache = self.cache.write().await;
            for (idx, embedding) in uncached_indices.iter().zip(new_embeddings.into_iter()) {
                cache.insert(texts[*idx].to_string(), embedding.clone());
                results[*idx] = Some(embedding);
            }
        }

        // All results should now be Some
        Ok(results.into_iter().map(|r| r.unwrap()).collect())
    }

    async fn embed_query(&self, text: &str) -> Result<Vec<f32>, SynapseError> {
        // Check cache
        {
            let cache = self.cache.read().await;
            if let Some(cached) = cache.get(text) {
                return Ok(cached.clone());
            }
        }

        // Cache miss: compute embedding
        let embedding = self.inner.embed_query(text).await?;

        // Store in cache
        {
            let mut cache = self.cache.write().await;
            cache.insert(text.to_string(), embedding.clone());
        }

        Ok(embedding)
    }
}
