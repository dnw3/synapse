//! SQLite integration for the Synaptic framework.
//!
//! This crate provides [`SqliteCache`], a SQLite-backed implementation of the
//! [`LlmCache`](synaptic_core::LlmCache) trait for caching LLM responses with
//! optional TTL expiration.
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

pub use cache::{SqliteCache, SqliteCacheConfig};

// Re-export core traits for convenience.
pub use synaptic_core::{ChatResponse, LlmCache};
