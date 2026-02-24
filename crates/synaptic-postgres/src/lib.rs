//! PostgreSQL integration for the Synaptic framework.
//!
//! This crate provides PostgreSQL-backed implementations of Synaptic traits:
//!
//! - [`PgVectorStore`] — [`VectorStore`](synaptic_core::VectorStore) using the
//!   [pgvector](https://github.com/pgvector/pgvector) extension for cosine-distance
//!   similarity search.
//! - [`PgStore`] — [`Store`](synaptic_core::Store) for key-value storage with JSONB
//!   values and optional full-text search via `tsvector`.
//! - [`PgCache`] — [`LlmCache`](synaptic_core::LlmCache) for caching LLM responses
//!   with optional TTL expiration.
//! - [`PgCheckpointer`] — Graph checkpoint persistence (requires `checkpointer` feature).
//!
//! # Quick start
//!
//! ```rust,no_run
//! use sqlx::postgres::PgPoolOptions;
//! use synaptic_postgres::{PgVectorConfig, PgVectorStore};
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

mod cache;
#[cfg(feature = "checkpointer")]
pub mod checkpointer;
mod store;
mod vector_store;

pub use cache::{PgCache, PgCacheConfig};
#[cfg(feature = "checkpointer")]
pub use checkpointer::PgCheckpointer;
pub use store::{PgStore, PgStoreConfig};
pub use vector_store::{PgVectorConfig, PgVectorStore};

// Re-export core traits/types for convenience.
pub use synaptic_core::{ChatResponse, Document, Embeddings, Item, LlmCache, Store, VectorStore};
