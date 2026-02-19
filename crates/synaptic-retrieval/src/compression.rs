use std::sync::Arc;

use async_trait::async_trait;
use synaptic_core::SynapticError;
use synaptic_embeddings::Embeddings;

use crate::{Document, Retriever};

/// Trait for compressing or filtering a set of documents based on a query.
#[async_trait]
pub trait DocumentCompressor: Send + Sync {
    /// Compress or filter documents based on relevance to the query.
    async fn compress_documents(
        &self,
        documents: Vec<Document>,
        query: &str,
    ) -> Result<Vec<Document>, SynapticError>;
}

/// Filters documents based on cosine similarity between the query embedding
/// and document content embeddings.
pub struct EmbeddingsFilter {
    embeddings: Arc<dyn Embeddings>,
    threshold: f32,
}

impl EmbeddingsFilter {
    /// Create a new EmbeddingsFilter with the given embeddings provider and similarity threshold.
    pub fn new(embeddings: Arc<dyn Embeddings>, threshold: f32) -> Self {
        Self {
            embeddings,
            threshold,
        }
    }

    /// Create a new EmbeddingsFilter with the default threshold of 0.75.
    pub fn with_default_threshold(embeddings: Arc<dyn Embeddings>) -> Self {
        Self::new(embeddings, 0.75)
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

#[async_trait]
impl DocumentCompressor for EmbeddingsFilter {
    async fn compress_documents(
        &self,
        documents: Vec<Document>,
        query: &str,
    ) -> Result<Vec<Document>, SynapticError> {
        if documents.is_empty() {
            return Ok(vec![]);
        }

        // Embed the query
        let query_embedding = self.embeddings.embed_query(query).await?;

        // Embed all document contents
        let doc_texts: Vec<&str> = documents.iter().map(|d| d.content.as_str()).collect();
        let doc_embeddings = self.embeddings.embed_documents(&doc_texts).await?;

        // Filter documents by cosine similarity threshold
        let filtered = documents
            .into_iter()
            .zip(doc_embeddings.iter())
            .filter(|(_, doc_emb)| cosine_similarity(&query_embedding, doc_emb) >= self.threshold)
            .map(|(doc, _)| doc)
            .collect();

        Ok(filtered)
    }
}

/// A retriever that retrieves documents from a base retriever and then
/// compresses/filters them using a DocumentCompressor.
pub struct ContextualCompressionRetriever {
    base: Arc<dyn Retriever>,
    compressor: Arc<dyn DocumentCompressor>,
}

impl ContextualCompressionRetriever {
    /// Create a new ContextualCompressionRetriever.
    pub fn new(base: Arc<dyn Retriever>, compressor: Arc<dyn DocumentCompressor>) -> Self {
        Self { base, compressor }
    }
}

#[async_trait]
impl Retriever for ContextualCompressionRetriever {
    async fn retrieve(&self, query: &str, top_k: usize) -> Result<Vec<Document>, SynapticError> {
        // First, retrieve from the base retriever
        let docs = self.base.retrieve(query, top_k).await?;

        // Then compress/filter the results
        let compressed = self.compressor.compress_documents(docs, query).await?;

        Ok(compressed)
    }
}
