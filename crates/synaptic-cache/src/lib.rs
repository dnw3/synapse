mod cached_model;
mod in_memory;
mod semantic;

pub use cached_model::CachedChatModel;
pub use in_memory::InMemoryCache;
pub use semantic::SemanticCache;

// Re-export LlmCache trait from core for backward compatibility
pub use synaptic_core::LlmCache;
