mod cached_model;
mod in_memory;
mod semantic;

pub use cached_model::CachedChatModel;
pub use in_memory::InMemoryCache;
pub use semantic::SemanticCache;

use async_trait::async_trait;
use synaptic_core::{ChatResponse, SynapticError};

/// Trait for caching LLM responses.
#[async_trait]
pub trait LlmCache: Send + Sync {
    /// Look up a cached response by cache key.
    async fn get(&self, key: &str) -> Result<Option<ChatResponse>, SynapticError>;
    /// Store a response in the cache.
    async fn put(&self, key: &str, response: &ChatResponse) -> Result<(), SynapticError>;
    /// Clear all entries from the cache.
    async fn clear(&self) -> Result<(), SynapticError>;
}
