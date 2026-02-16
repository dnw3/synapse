use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use synapse_core::SynapseError;
use synapse_embeddings::Embeddings;
use synapse_retrieval::{Document, Retriever};
use tokio::sync::RwLock;

use crate::VectorStore;

/// Stored document with its embedding vector.
struct StoredEntry {
    document: Document,
    embedding: Vec<f32>,
}

/// In-memory vector store using cosine similarity.
pub struct InMemoryVectorStore {
    entries: RwLock<HashMap<String, StoredEntry>>,
}

impl InMemoryVectorStore {
    pub fn new() -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
        }
    }
}

impl Default for InMemoryVectorStore {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl VectorStore for InMemoryVectorStore {
    async fn add_documents(
        &self,
        docs: Vec<Document>,
        embeddings: &dyn Embeddings,
    ) -> Result<Vec<String>, SynapseError> {
        let texts: Vec<&str> = docs.iter().map(|d| d.content.as_str()).collect();
        let vectors = embeddings.embed_documents(&texts).await?;

        let mut entries = self.entries.write().await;
        let mut ids = Vec::with_capacity(docs.len());

        for (doc, embedding) in docs.into_iter().zip(vectors) {
            ids.push(doc.id.clone());
            entries.insert(
                doc.id.clone(),
                StoredEntry {
                    document: doc,
                    embedding,
                },
            );
        }

        Ok(ids)
    }

    async fn similarity_search(
        &self,
        query: &str,
        k: usize,
        embeddings: &dyn Embeddings,
    ) -> Result<Vec<Document>, SynapseError> {
        let results = self
            .similarity_search_with_score(query, k, embeddings)
            .await?;
        Ok(results.into_iter().map(|(doc, _)| doc).collect())
    }

    async fn similarity_search_with_score(
        &self,
        query: &str,
        k: usize,
        embeddings: &dyn Embeddings,
    ) -> Result<Vec<(Document, f32)>, SynapseError> {
        let query_vec = embeddings.embed_query(query).await?;
        let entries = self.entries.read().await;

        let mut scored: Vec<(Document, f32)> = entries
            .values()
            .map(|entry| {
                let score = cosine_similarity(&query_vec, &entry.embedding);
                (entry.document.clone(), score)
            })
            .collect();

        // Sort by score descending
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(k);

        Ok(scored)
    }

    async fn delete(&self, ids: &[&str]) -> Result<(), SynapseError> {
        let mut entries = self.entries.write().await;
        for id in ids {
            entries.remove(*id);
        }
        Ok(())
    }
}

/// A retriever that wraps a VectorStore, bridging it to the `Retriever` trait.
pub struct VectorStoreRetriever<S: VectorStore> {
    store: Arc<S>,
    embeddings: Arc<dyn Embeddings>,
    k: usize,
}

impl<S: VectorStore + 'static> VectorStoreRetriever<S> {
    pub fn new(store: Arc<S>, embeddings: Arc<dyn Embeddings>, k: usize) -> Self {
        Self {
            store,
            embeddings,
            k,
        }
    }
}

#[async_trait]
impl<S: VectorStore + 'static> Retriever for VectorStoreRetriever<S> {
    async fn retrieve(&self, query: &str, top_k: usize) -> Result<Vec<Document>, SynapseError> {
        let k = if top_k > 0 { top_k } else { self.k };
        self.store
            .similarity_search(query, k, self.embeddings.as_ref())
            .await
    }
}

/// Compute cosine similarity between two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    if a.len() != b.len() || a.is_empty() {
        return 0.0;
    }

    let dot: f32 = a.iter().zip(b.iter()).map(|(x, y)| x * y).sum();
    let mag_a: f32 = a.iter().map(|x| x * x).sum::<f32>().sqrt();
    let mag_b: f32 = b.iter().map(|x| x * x).sum::<f32>().sqrt();

    if mag_a == 0.0 || mag_b == 0.0 {
        return 0.0;
    }

    dot / (mag_a * mag_b)
}
