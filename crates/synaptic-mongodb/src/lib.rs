//! MongoDB Atlas Vector Search integration for Synaptic.
//!
//! This crate provides [`MongoVectorStore`], an implementation of the
//! [`VectorStore`](synaptic_core::VectorStore) trait backed by
//! [MongoDB Atlas Vector Search](https://www.mongodb.com/docs/atlas/atlas-vector-search/).
//!
//! # Example
//!
//! ```rust,no_run
//! use synaptic_mongodb::{MongoVectorStore, MongoVectorConfig};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = MongoVectorConfig::new("my_database", "my_collection");
//! let store = MongoVectorStore::from_uri("mongodb+srv://...", config).await?;
//! # Ok(())
//! # }
//! ```

mod vector_store;

pub use vector_store::{MongoVectorConfig, MongoVectorStore};

// Re-export core traits for convenience.
pub use synaptic_core::{Document, Embeddings, VectorStore};
