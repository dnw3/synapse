use std::collections::HashMap;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use synaptic_core::{ChatResponse, SynapseError};
use tokio::sync::RwLock;

use crate::LlmCache;

struct CacheEntry {
    response: ChatResponse,
    created_at: Instant,
}

/// In-memory LLM response cache with optional TTL expiration.
pub struct InMemoryCache {
    store: RwLock<HashMap<String, CacheEntry>>,
    ttl: Option<Duration>,
}

impl InMemoryCache {
    /// Create a new cache with no TTL (entries never expire).
    pub fn new() -> Self {
        Self {
            store: RwLock::new(HashMap::new()),
            ttl: None,
        }
    }

    /// Create a new cache where entries expire after the given duration.
    pub fn with_ttl(duration: Duration) -> Self {
        Self {
            store: RwLock::new(HashMap::new()),
            ttl: Some(duration),
        }
    }
}

impl Default for InMemoryCache {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl LlmCache for InMemoryCache {
    async fn get(&self, key: &str) -> Result<Option<ChatResponse>, SynapseError> {
        let store = self.store.read().await;
        match store.get(key) {
            Some(entry) => {
                if let Some(ttl) = self.ttl {
                    if entry.created_at.elapsed() > ttl {
                        return Ok(None);
                    }
                }
                Ok(Some(entry.response.clone()))
            }
            None => Ok(None),
        }
    }

    async fn put(&self, key: &str, response: &ChatResponse) -> Result<(), SynapseError> {
        let mut store = self.store.write().await;
        store.insert(
            key.to_string(),
            CacheEntry {
                response: response.clone(),
                created_at: Instant::now(),
            },
        );
        Ok(())
    }

    async fn clear(&self) -> Result<(), SynapseError> {
        let mut store = self.store.write().await;
        store.clear();
        Ok(())
    }
}
