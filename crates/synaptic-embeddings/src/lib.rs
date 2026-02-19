mod cached;
mod fake;
mod ollama;
mod openai;

pub use cached::CacheBackedEmbeddings;
pub use fake::FakeEmbeddings;
pub use ollama::{OllamaEmbeddings, OllamaEmbeddingsConfig};
pub use openai::{OpenAiEmbeddings, OpenAiEmbeddingsConfig};

// Re-export the Embeddings trait from core (forward-declared there).
pub use synaptic_core::Embeddings;
