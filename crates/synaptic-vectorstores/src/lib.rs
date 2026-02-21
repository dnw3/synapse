mod in_memory;
mod multi_vector;

pub use in_memory::{InMemoryVectorStore, VectorStoreRetriever};
pub use multi_vector::MultiVectorRetriever;

// Re-export core traits/types for backward compatibility
pub use synaptic_core::{Document, Embeddings, Retriever, VectorStore};
