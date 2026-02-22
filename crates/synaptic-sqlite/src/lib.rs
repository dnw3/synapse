//! SQLite integration for the Synaptic framework.
//!
//! This crate provides:
//! - [`SqliteCache`]: A SQLite-backed implementation of the [`LlmCache`](synaptic_core::LlmCache)
//!   trait for caching LLM responses with optional TTL expiration.
//! - [`SqliteCheckpointer`]: A SQLite-backed implementation of the
//!   [`Checkpointer`](synaptic_graph::Checkpointer) trait for persisting graph
//!   state between executions.
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

pub use cache::{SqliteCache, SqliteCacheConfig};
pub use checkpointer::SqliteCheckpointer;

// Re-export core traits for convenience.
pub use synaptic_core::{ChatResponse, LlmCache};
pub use synaptic_graph::Checkpointer;
