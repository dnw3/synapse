//! ChromaDB vector store integration for Synaptic.
//!
//! This crate provides [`ChromaVectorStore`], an implementation of the
//! [`VectorStore`](synaptic_core::VectorStore) trait backed by
//! [ChromaDB](https://www.trychroma.com/) using its REST API.
//!
//! # Example
//!
//! ```rust,no_run
//! use synaptic_chroma::{ChromaVectorStore, ChromaConfig};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = ChromaConfig::new("my_collection");
//! let store = ChromaVectorStore::new(config);
//! store.ensure_collection().await?;
//! # Ok(())
//! # }
//! ```

mod vector_store;

pub use vector_store::{ChromaConfig, ChromaVectorStore};

// Re-export core traits for convenience.
pub use synaptic_core::{Document, Embeddings, VectorStore};
