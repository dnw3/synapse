use std::sync::Arc;
use std::time::Duration;

use synaptic_cache::{InMemoryCache, LlmCache};
use synaptic_core::{ChatResponse, Message};

fn make_response(text: &str) -> ChatResponse {
    ChatResponse {
        message: Message::ai(text),
        usage: None,
    }
}

#[tokio::test]
async fn cache_hit() {
    let cache = InMemoryCache::new();
    let key = "test_key";
    cache.put(key, &make_response("cached")).await.unwrap();

    let result = cache.get(key).await.unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap().message.content(), "cached");
}

#[tokio::test]
async fn cache_miss() {
    let cache = InMemoryCache::new();
    let result = cache.get("nonexistent").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn ttl_expired() {
    let cache = InMemoryCache::with_ttl(Duration::from_millis(50));
    cache
        .put("key", &make_response("expiring"))
        .await
        .unwrap();

    // Should be cached
    let result = cache.get("key").await.unwrap();
    assert!(result.is_some());

    // Wait for expiration
    tokio::time::sleep(Duration::from_millis(100)).await;

    let result = cache.get("key").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn no_ttl_never_expires() {
    let cache = InMemoryCache::new();
    cache
        .put("key", &make_response("persistent"))
        .await
        .unwrap();

    // Wait a bit
    tokio::time::sleep(Duration::from_millis(50)).await;

    let result = cache.get("key").await.unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap().message.content(), "persistent");
}

#[tokio::test]
async fn put_get_roundtrip() {
    let cache = InMemoryCache::new();
    let response = make_response("roundtrip");
    cache.put("rk", &response).await.unwrap();

    let retrieved = cache.get("rk").await.unwrap().unwrap();
    assert_eq!(retrieved.message.content(), "roundtrip");
}

#[tokio::test]
async fn concurrent_access() {
    let cache = Arc::new(InMemoryCache::new());
    let mut handles = Vec::new();

    // Write from multiple tasks
    for i in 0..10 {
        let c = cache.clone();
        handles.push(tokio::spawn(async move {
            let key = format!("key_{i}");
            c.put(&key, &make_response(&format!("val_{i}")))
                .await
                .unwrap();
        }));
    }

    for h in handles {
        h.await.unwrap();
    }

    // Verify all were written
    for i in 0..10 {
        let key = format!("key_{i}");
        let result = cache.get(&key).await.unwrap();
        assert!(result.is_some(), "key_{i} should exist");
    }
}

#[tokio::test]
async fn clear_removes_all() {
    let cache = InMemoryCache::new();
    cache.put("a", &make_response("va")).await.unwrap();
    cache.put("b", &make_response("vb")).await.unwrap();

    cache.clear().await.unwrap();

    assert!(cache.get("a").await.unwrap().is_none());
    assert!(cache.get("b").await.unwrap().is_none());
}

#[tokio::test]
async fn overwrite_existing_key() {
    let cache = InMemoryCache::new();
    cache.put("k", &make_response("old")).await.unwrap();
    cache.put("k", &make_response("new")).await.unwrap();

    let result = cache.get("k").await.unwrap().unwrap();
    assert_eq!(result.message.content(), "new");
}
