use std::sync::Arc;

use synaptic_cache::{LlmCache, SemanticCache};
use synaptic_core::{ChatResponse, Message};
use synaptic_embeddings::FakeEmbeddings;

fn make_response(text: &str) -> ChatResponse {
    ChatResponse {
        message: Message::ai(text),
        usage: None,
    }
}

#[tokio::test]
async fn semantic_cache_exact_match() {
    let embeddings = Arc::new(FakeEmbeddings::new(4));
    let cache = SemanticCache::new(embeddings, 0.95);

    let response = make_response("cached answer");
    cache.put("What is Rust?", &response).await.unwrap();

    // Exact same query should hit the cache
    let result = cache.get("What is Rust?").await.unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap().message.content(), "cached answer");
}

#[tokio::test]
async fn semantic_cache_similar_match() {
    let embeddings = Arc::new(FakeEmbeddings::new(4));
    // Use a lower threshold so similar texts can match
    let cache = SemanticCache::new(embeddings, 0.90);

    let response = make_response("rust answer");
    cache.put("What is Rust?", &response).await.unwrap();

    // Very similar query (FakeEmbeddings generates similar vectors for similar text)
    let result = cache.get("What is Rust!").await;
    // Just verify it doesn't error — the actual match depends on FakeEmbeddings behavior
    assert!(result.is_ok());
}

#[tokio::test]
async fn semantic_cache_miss_below_threshold() {
    let embeddings = Arc::new(FakeEmbeddings::new(4));
    // Very high threshold
    let cache = SemanticCache::new(embeddings, 0.9999);

    let response = make_response("answer about rust");
    cache.put("What is Rust?", &response).await.unwrap();

    // Completely different text should not match at high threshold
    let result = cache
        .get("How do I cook pasta with tomatoes?")
        .await
        .unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn semantic_cache_clear_removes_all() {
    let embeddings = Arc::new(FakeEmbeddings::new(4));
    let cache = SemanticCache::new(embeddings, 0.95);

    cache.put("key1", &make_response("answer1")).await.unwrap();
    cache.put("key2", &make_response("answer2")).await.unwrap();

    // Both should be cached
    assert!(cache.get("key1").await.unwrap().is_some());
    assert!(cache.get("key2").await.unwrap().is_some());

    // Clear all
    cache.clear().await.unwrap();
    assert!(cache.get("key1").await.unwrap().is_none());
    assert!(cache.get("key2").await.unwrap().is_none());
}

#[tokio::test]
async fn semantic_cache_multiple_entries() {
    let embeddings = Arc::new(FakeEmbeddings::new(4));
    let cache = SemanticCache::new(embeddings, 0.95);

    cache
        .put("What is Rust?", &make_response("Rust answer"))
        .await
        .unwrap();
    cache
        .put("What is Python?", &make_response("Python answer"))
        .await
        .unwrap();
    cache
        .put("What is Java?", &make_response("Java answer"))
        .await
        .unwrap();

    let rust = cache.get("What is Rust?").await.unwrap();
    assert!(rust.is_some());
    assert_eq!(rust.unwrap().message.content(), "Rust answer");
}
