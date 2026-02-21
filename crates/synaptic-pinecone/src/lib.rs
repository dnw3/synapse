//! Pinecone vector store integration for Synaptic.
//!
//! This crate provides [`PineconeVectorStore`], an implementation of the
//! [`VectorStore`](synaptic_core::VectorStore) trait backed by
//! [Pinecone](https://www.pinecone.io/) using its REST API.
//!
//! # Example
//!
//! ```rust,no_run
//! use synaptic_pinecone::{PineconeVectorStore, PineconeConfig};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = PineconeConfig::new("your-api-key", "https://my-index-abc123.svc.pinecone.io");
//! let store = PineconeVectorStore::new(config);
//! # Ok(())
//! # }
//! ```

mod vector_store;

pub use vector_store::{PineconeConfig, PineconeVectorStore};

// Re-export core traits for convenience.
pub use synaptic_core::{Document, Embeddings, VectorStore};
