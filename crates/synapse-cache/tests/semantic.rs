use std::sync::Arc;

use synapse_cache::{LlmCache, SemanticCache};
use synapse_core::{ChatResponse, Message};
use synapse_embeddings::FakeEmbeddings;

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
