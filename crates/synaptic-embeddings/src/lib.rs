mod cached;
mod fake;

pub use cached::CacheBackedEmbeddings;
pub use fake::FakeEmbeddings;

// Re-export the Embeddings trait from core (forward-declared there).
pub use synaptic_core::Embeddings;
