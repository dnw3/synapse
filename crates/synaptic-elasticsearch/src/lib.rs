//! Elasticsearch vector store integration for Synaptic.
//!
//! This crate provides [`ElasticsearchVectorStore`], an implementation of the
//! [`VectorStore`](synaptic_core::VectorStore) trait backed by
//! [Elasticsearch](https://www.elastic.co/elasticsearch/) using its REST API
//! with `dense_vector` fields and kNN search.
//!
//! # Example
//!
//! ```rust,no_run
//! use synaptic_elasticsearch::{ElasticsearchVectorStore, ElasticsearchConfig};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = ElasticsearchConfig::new("my_documents", 1536);
//! let store = ElasticsearchVectorStore::new(config);
//! store.ensure_index().await?;
//! # Ok(())
//! # }
//! ```

mod vector_store;

pub use vector_store::{ElasticsearchConfig, ElasticsearchVectorStore};

// Re-export core traits for convenience.
pub use synaptic_core::{Document, Embeddings, VectorStore};
