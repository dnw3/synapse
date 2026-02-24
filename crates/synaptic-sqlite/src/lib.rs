//! SQLite integration for the Synaptic framework.
//!
//! This crate provides:
//! - [`SqliteCache`]: A SQLite-backed implementation of the [`LlmCache`](synaptic_core::LlmCache)
//!   trait for caching LLM responses with optional TTL expiration.
//! - [`SqliteCheckpointer`]: A SQLite-backed implementation of the
//!   [`Checkpointer`](synaptic_graph::Checkpointer) trait for persisting graph
//!   state between executions.
//! - [`SqliteStore`]: A SQLite-backed implementation of the [`Store`](synaptic_core::Store)
//!   trait with FTS5 full-text search.
//! - [`SqliteVectorStore`]: A SQLite-backed implementation of the
//!   [`VectorStore`](synaptic_core::VectorStore) trait with FTS5 hybrid search.
//!
//! # Quick start
//!
//! ```rust,no_run
//! use synaptic_sqlite::{SqliteCache, SqliteCacheConfig};
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // In-memory cache (great for testing)
//! let cache = SqliteCache::new(SqliteCacheConfig::in_memory())?;
//!
//! // File-based cache with 1-hour TTL
//! let config = SqliteCacheConfig::new("/tmp/llm_cache.db").with_ttl(3600);
//! let cache = SqliteCache::new(config)?;
//! # Ok(())
//! # }
//! ```

mod cache;
pub mod checkpointer;
mod store;
mod vectorstore;

pub use cache::{SqliteCache, SqliteCacheConfig};
pub use checkpointer::SqliteCheckpointer;
pub use store::{SqliteStore, SqliteStoreConfig};
pub use vectorstore::{SqliteVectorStore, SqliteVectorStoreConfig};

// Re-export core traits for convenience.
pub use synaptic_core::{ChatResponse, Document, Embeddings, Item, LlmCache, Store, VectorStore};
pub use synaptic_graph::Checkpointer;
