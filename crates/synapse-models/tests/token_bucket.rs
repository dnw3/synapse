use std::sync::Arc;
use std::time::Instant;

use synapse_core::{ChatModel, ChatRequest, ChatResponse, Message, SynapseError};
use synapse_models::TokenBucketChatModel;

struct InstantModel;

#[async_trait::async_trait]
impl ChatModel for InstantModel {
    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, SynapseError> {
        Ok(ChatResponse {
            message: Message::ai("ok"),
            usage: None,
        })
    }
}

#[tokio::test]
async fn token_bucket_allows_immediate_when_available() {
    // Capacity of 5 tokens, refill 10/sec — plenty of tokens available
    let inner = Arc::new(InstantModel);
    let model = TokenBucketChatModel::new(inner, 5.0, 10.0);

    let start = Instant::now();

    // Three rapid calls should go through immediately (bucket starts full at 5)
    for _ in 0..3 {
        let result = model
            .chat(ChatRequest::new(vec![Message::human("hi")]))
            .await;
        assert!(result.is_ok());
    }

    // Should complete almost instantly
    assert!(start.elapsed().as_millis() < 100);
}

#[tokio::test]
async fn token_bucket_limits_rate() {
    // Capacity of 1 token, refill 10/sec — second request must wait ~100ms
    let inner = Arc::new(InstantModel);
    let model = Arc::new(TokenBucketChatModel::new(inner, 1.0, 10.0));

    let start = Instant::now();

    // First call uses the one available token immediately
    model
        .chat(ChatRequest::new(vec![Message::human("a")]))
        .await
        .unwrap();

    // Second call must wait for refill (~100ms for 1 token at 10/sec)
    model
        .chat(ChatRequest::new(vec![Message::human("b")]))
        .await
        .unwrap();

    // Should have taken at least ~80ms (allowing some timing slack)
    assert!(start.elapsed().as_millis() >= 50);
}

#[tokio::test]
async fn token_bucket_refills() {
    // Capacity of 1 token, refill 20/sec
    let inner = Arc::new(InstantModel);
    let model = TokenBucketChatModel::new(inner, 1.0, 20.0);

    // Use the one available token
    model
        .chat(ChatRequest::new(vec![Message::human("first")]))
        .await
        .unwrap();

    // Wait long enough for a token to refill (50ms at 20/sec = 1 token)
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;

    // Should now have a token available and complete quickly
    let start = Instant::now();
    model
        .chat(ChatRequest::new(vec![Message::human("second")]))
        .await
        .unwrap();

    assert!(start.elapsed().as_millis() < 100);
}
