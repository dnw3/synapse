//! Qdrant vector store integration for Synaptic.
//!
//! This crate provides [`QdrantVectorStore`], an implementation of the
//! [`VectorStore`](synaptic_core::VectorStore) trait backed by [Qdrant](https://qdrant.tech/).
//!
//! # Example
//!
//! ```rust,no_run
//! use synaptic_qdrant::{QdrantVectorStore, QdrantConfig};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = QdrantConfig::new("http://localhost:6334", "my_collection", 1536);
//! let store = QdrantVectorStore::new(config)?;
//! # Ok(())
//! # }
//! ```

mod vector_store;

pub use vector_store::{QdrantConfig, QdrantVectorStore};

// Re-export core traits for convenience.
pub use synaptic_core::{Document, Embeddings, VectorStore};
