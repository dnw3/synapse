//! PostgreSQL + pgvector integration for the Synaptic framework.
//!
//! This crate provides [`PgVectorStore`], an implementation of the
//! [`VectorStore`](synaptic_core::VectorStore) trait backed by PostgreSQL with
//! the [pgvector](https://github.com/pgvector/pgvector) extension. It stores
//! document content, metadata (as JSONB), and embedding vectors in a single
//! table, using cosine distance (`<=>`) for similarity search.
//!
//! # Quick start
//!
//! ```rust,no_run
//! use sqlx::postgres::PgPoolOptions;
//! use synaptic_pgvector::{PgVectorConfig, PgVectorStore};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let pool = PgPoolOptions::new()
//!     .max_connections(5)
//!     .connect("postgres://user:pass@localhost/mydb")
//!     .await?;
//!
//! let config = PgVectorConfig::new("documents", 1536);
//! let store = PgVectorStore::new(pool, config);
//! store.initialize().await?;
//! # Ok(())
//! # }
//! ```

mod vector_store;

pub use vector_store::{PgVectorConfig, PgVectorStore};

// Re-export core traits/types for convenience.
pub use synaptic_core::{Document, Embeddings, VectorStore};
