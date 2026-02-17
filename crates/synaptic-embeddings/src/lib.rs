mod cached;
mod fake;
mod ollama;
mod openai;

pub use cached::CacheBackedEmbeddings;
pub use fake::FakeEmbeddings;
pub use ollama::{OllamaEmbeddings, OllamaEmbeddingsConfig};
pub use openai::{OpenAiEmbeddings, OpenAiEmbeddingsConfig};

use async_trait::async_trait;
use synaptic_core::SynapseError;

/// Trait for embedding text into vectors.
#[async_trait]
pub trait Embeddings: Send + Sync {
    /// Embed multiple texts (for batch document embedding).
    async fn embed_documents(&self, texts: &[&str]) -> Result<Vec<Vec<f32>>, SynapseError>;

    /// Embed a single query text.
    async fn embed_query(&self, text: &str) -> Result<Vec<f32>, SynapseError>;
}
