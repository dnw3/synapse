mod in_memory;
mod multi_vector;

pub use in_memory::{InMemoryVectorStore, VectorStoreRetriever};
pub use multi_vector::MultiVectorRetriever;

use async_trait::async_trait;
use synapse_core::SynapseError;
use synapse_embeddings::Embeddings;
use synapse_retrieval::Document;

/// Trait for vector storage backends.
#[async_trait]
pub trait VectorStore: Send + Sync {
    /// Add documents to the store, computing their embeddings.
    async fn add_documents(
        &self,
        docs: Vec<Document>,
        embeddings: &dyn Embeddings,
    ) -> Result<Vec<String>, SynapseError>;

    /// Search for similar documents by query string.
    async fn similarity_search(
        &self,
        query: &str,
        k: usize,
        embeddings: &dyn Embeddings,
    ) -> Result<Vec<Document>, SynapseError>;

    /// Search with similarity scores (higher = more similar).
    async fn similarity_search_with_score(
        &self,
        query: &str,
        k: usize,
        embeddings: &dyn Embeddings,
    ) -> Result<Vec<(Document, f32)>, SynapseError>;

    /// Search by pre-computed embedding vector instead of text query.
    async fn similarity_search_by_vector(
        &self,
        embedding: &[f32],
        k: usize,
    ) -> Result<Vec<Document>, SynapseError>;

    /// Delete documents by ID.
    async fn delete(&self, ids: &[&str]) -> Result<(), SynapseError>;
}
