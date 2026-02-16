mod cached_model;
mod in_memory;
mod semantic;

pub use cached_model::CachedChatModel;
pub use in_memory::InMemoryCache;
pub use semantic::SemanticCache;

use async_trait::async_trait;
use synapse_core::{ChatResponse, SynapseError};

/// Trait for caching LLM responses.
#[async_trait]
pub trait LlmCache: Send + Sync {
    /// Look up a cached response by cache key.
    async fn get(&self, key: &str) -> Result<Option<ChatResponse>, SynapseError>;
    /// Store a response in the cache.
    async fn put(&self, key: &str, response: &ChatResponse) -> Result<(), SynapseError>;
}
