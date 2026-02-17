use std::time::Duration;

use synapse_cache::{InMemoryCache, LlmCache};
use synapse_core::{ChatResponse, Message};

fn make_response(text: &str) -> ChatResponse {
    ChatResponse {
        message: Message::ai(text),
        usage: None,
    }
}

#[tokio::test]
async fn cache_stores_and_retrieves() {
    let cache = InMemoryCache::new();
    let response = make_response("hello");

    cache.put("key1", &response).await.unwrap();
    let result = cache.get("key1").await.unwrap();

    assert!(result.is_some());
    assert_eq!(result.unwrap().message.content(), "hello");
}

#[tokio::test]
async fn cache_returns_none_for_miss() {
    let cache = InMemoryCache::new();
    let result = cache.get("nonexistent").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn cache_ttl_expires() {
    let cache = InMemoryCache::with_ttl(Duration::from_millis(50));
    let response = make_response("ephemeral");

    cache.put("key1", &response).await.unwrap();

    // Should be present immediately
    assert!(cache.get("key1").await.unwrap().is_some());

    // Wait for TTL to expire
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Should be expired now
    assert!(cache.get("key1").await.unwrap().is_none());
}

#[tokio::test]
async fn cache_no_ttl_persists() {
    let cache = InMemoryCache::new();
    let response = make_response("persistent");

    cache.put("key1", &response).await.unwrap();

    // Wait a bit
    tokio::time::sleep(Duration::from_millis(50)).await;

    // Should still be present
    let result = cache.get("key1").await.unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap().message.content(), "persistent");
}

#[tokio::test]
async fn cache_clear_removes_all_entries() {
    let cache = InMemoryCache::new();

    cache.put("key1", &make_response("a")).await.unwrap();
    cache.put("key2", &make_response("b")).await.unwrap();

    // Both should be present
    assert!(cache.get("key1").await.unwrap().is_some());
    assert!(cache.get("key2").await.unwrap().is_some());

    // Clear the cache
    cache.clear().await.unwrap();

    // Both should be gone
    assert!(cache.get("key1").await.unwrap().is_none());
    assert!(cache.get("key2").await.unwrap().is_none());
}
