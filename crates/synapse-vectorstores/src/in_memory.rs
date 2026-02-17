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

    /// Create a new store pre-populated with texts.
    pub async fn from_texts(
        texts: Vec<(&str, &str)>,
        embeddings: &dyn Embeddings,
    ) -> Result<Self, SynapseError> {
        let store = Self::new();
        let docs = texts
            .into_iter()
            .map(|(id, content)| Document::new(id, content))
            .collect();
        store.add_documents(docs, embeddings).await?;
        Ok(store)
    }

    /// Create a new store pre-populated with documents.
    pub async fn from_documents(
        documents: Vec<Document>,
        embeddings: &dyn Embeddings,
    ) -> Result<Self, SynapseError> {
        let store = Self::new();
        store.add_documents(documents, embeddings).await?;
        Ok(store)
    }

    /// Maximum Marginal Relevance search for diverse results.
    ///
    /// `lambda_mult` controls the trade-off between relevance and diversity:
    /// - 1.0 = pure relevance (equivalent to standard similarity search)
    /// - 0.0 = maximum diversity
    /// - 0.5 = balanced (typical default)
    ///
    /// `fetch_k` is the number of initial candidates to fetch before MMR filtering.
    pub async fn max_marginal_relevance_search(
        &self,
        query: &str,
        k: usize,
        fetch_k: usize,
        lambda_mult: f32,
        embeddings: &dyn Embeddings,
    ) -> Result<Vec<Document>, SynapseError> {
        let query_vec = embeddings.embed_query(query).await?;
        let entries = self.entries.read().await;

        // Score all candidates against the query
        let mut candidates: Vec<(String, Document, Vec<f32>, f32)> = entries
            .values()
            .map(|entry| {
                let score = cosine_similarity(&query_vec, &entry.embedding);
                (
                    entry.document.id.clone(),
                    entry.document.clone(),
                    entry.embedding.clone(),
                    score,
                )
            })
            .collect();

        // Sort by query similarity descending and take top fetch_k
        candidates.sort_by(|a, b| b.3.partial_cmp(&a.3).unwrap_or(std::cmp::Ordering::Equal));
        candidates.truncate(fetch_k);

        if candidates.is_empty() || k == 0 {
            return Ok(Vec::new());
        }

        // Greedy MMR selection
        let mut selected: Vec<(Document, Vec<f32>)> = Vec::with_capacity(k);
        let mut remaining = candidates;

        while selected.len() < k && !remaining.is_empty() {
            let mut best_idx = 0;
            let mut best_score = f32::NEG_INFINITY;

            for (i, (_id, _doc, emb, query_sim)) in remaining.iter().enumerate() {
                // Compute max similarity to already-selected documents
                let max_sim_to_selected = if selected.is_empty() {
                    0.0
                } else {
                    selected
                        .iter()
                        .map(|(_, sel_emb)| cosine_similarity(sel_emb, emb))
                        .fold(f32::NEG_INFINITY, f32::max)
                };

                let mmr_score = lambda_mult * query_sim - (1.0 - lambda_mult) * max_sim_to_selected;

                if mmr_score > best_score {
                    best_score = mmr_score;
                    best_idx = i;
                }
            }

            let (_id, doc, emb, _query_sim) = remaining.remove(best_idx);
            selected.push((doc, emb));
        }

        Ok(selected.into_iter().map(|(doc, _)| doc).collect())
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

    async fn similarity_search_by_vector(
        &self,
        embedding: &[f32],
        k: usize,
    ) -> Result<Vec<Document>, SynapseError> {
        let entries = self.entries.read().await;

        let mut scored: Vec<(Document, f32)> = entries
            .values()
            .map(|entry| {
                let score = cosine_similarity(embedding, &entry.embedding);
                (entry.document.clone(), score)
            })
            .collect();

        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(k);

        Ok(scored.into_iter().map(|(doc, _)| doc).collect())
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
    score_threshold: Option<f32>,
}

impl<S: VectorStore + 'static> VectorStoreRetriever<S> {
    pub fn new(store: Arc<S>, embeddings: Arc<dyn Embeddings>, k: usize) -> Self {
        Self {
            store,
            embeddings,
            k,
            score_threshold: None,
        }
    }

    /// Set a minimum similarity score threshold. Only documents with a score
    /// greater than or equal to the threshold will be returned.
    pub fn with_score_threshold(mut self, threshold: f32) -> Self {
        self.score_threshold = Some(threshold);
        self
    }
}

#[async_trait]
impl<S: VectorStore + 'static> Retriever for VectorStoreRetriever<S> {
    async fn retrieve(&self, query: &str, top_k: usize) -> Result<Vec<Document>, SynapseError> {
        let k = if top_k > 0 { top_k } else { self.k };

        if let Some(threshold) = self.score_threshold {
            let scored = self
                .store
                .similarity_search_with_score(query, k, self.embeddings.as_ref())
                .await?;
            Ok(scored
                .into_iter()
                .filter(|(_, score)| *score >= threshold)
                .map(|(doc, _)| doc)
                .collect())
        } else {
            self.store
                .similarity_search(query, k, self.embeddings.as_ref())
                .await
        }
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
