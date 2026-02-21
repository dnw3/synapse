//! Redis integration for the Synaptic framework.
//!
//! This crate provides two Redis-backed implementations:
//!
//! - [`RedisStore`] — implements the [`Store`](synaptic_core::Store) trait for
//!   persistent key-value storage with namespace support.
//! - [`RedisCache`] — implements the [`LlmCache`](synaptic_core::LlmCache) trait
//!   for caching LLM responses with optional TTL expiration.
//!
//! # Quick start
//!
//! ```rust,no_run
//! use synaptic_redis::{RedisStore, RedisStoreConfig, RedisCache, RedisCacheConfig};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! // Store
//! let store = RedisStore::from_url("redis://127.0.0.1/")?;
//!
//! // Cache with 1-hour TTL
//! let config = RedisCacheConfig { ttl: Some(3600), ..Default::default() };
//! let cache = RedisCache::from_url_with_config("redis://127.0.0.1/", config)?;
//! # Ok(())
//! # }
//! ```

mod cache;
mod store;

pub use cache::{RedisCache, RedisCacheConfig};
pub use store::{RedisStore, RedisStoreConfig};

// Re-export core traits for convenience.
pub use synaptic_core::{ChatResponse, Item, LlmCache, Store};
