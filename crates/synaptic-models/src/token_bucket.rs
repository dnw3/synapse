use std::sync::Arc;

use async_trait::async_trait;
use synaptic_core::{ChatModel, ChatRequest, ChatResponse, ChatStream, SynapseError};
use tokio::sync::Mutex;
use tokio::time::Instant;

/// A token bucket rate limiter.
///
/// Starts full at `capacity` tokens and refills at `refill_rate` tokens per second.
/// Calling [`acquire`](TokenBucket::acquire) waits until a token is available, then
/// consumes one token.
pub struct TokenBucket {
    capacity: f64,
    tokens: Mutex<f64>,
    refill_rate: f64,
    last_refill: Mutex<Instant>,
}

impl TokenBucket {
    /// Create a new token bucket that starts full.
    ///
    /// - `capacity`: maximum number of tokens the bucket can hold
    /// - `refill_rate`: tokens added per second
    pub fn new(capacity: f64, refill_rate: f64) -> Self {
        Self {
            capacity,
            tokens: Mutex::new(capacity),
            refill_rate,
            last_refill: Mutex::new(Instant::now()),
        }
    }

    /// Wait until a token is available and consume it.
    pub async fn acquire(&self) {
        loop {
            self.refill().await;

            let mut tokens = self.tokens.lock().await;
            if *tokens >= 1.0 {
                *tokens -= 1.0;
                return;
            }
            drop(tokens);

            // Wait a short interval before checking again
            // Calculate how long until we have at least 1 token
            let wait = std::time::Duration::from_secs_f64(1.0 / self.refill_rate);
            tokio::time::sleep(wait).await;
        }
    }

    async fn refill(&self) {
        let now = Instant::now();
        let mut last_refill = self.last_refill.lock().await;
        let elapsed = now.duration_since(*last_refill);
        let new_tokens = elapsed.as_secs_f64() * self.refill_rate;

        if new_tokens > 0.0 {
            let mut tokens = self.tokens.lock().await;
            *tokens = (*tokens + new_tokens).min(self.capacity);
            *last_refill = now;
        }
    }
}

/// A ChatModel wrapper that uses a [`TokenBucket`] to rate-limit requests.
///
/// Each call to `chat` or `stream_chat` acquires one token before delegating
/// to the inner model.
pub struct TokenBucketChatModel {
    inner: Arc<dyn ChatModel>,
    bucket: Arc<TokenBucket>,
}

impl TokenBucketChatModel {
    /// Create a new token-bucket rate-limited model.
    ///
    /// - `inner`: the model to wrap
    /// - `capacity`: maximum burst size (tokens)
    /// - `refill_rate`: tokens per second
    pub fn new(inner: Arc<dyn ChatModel>, capacity: f64, refill_rate: f64) -> Self {
        Self {
            inner,
            bucket: Arc::new(TokenBucket::new(capacity, refill_rate)),
        }
    }
}

#[async_trait]
impl ChatModel for TokenBucketChatModel {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, SynapseError> {
        self.bucket.acquire().await;
        self.inner.chat(request).await
    }

    fn stream_chat(&self, request: ChatRequest) -> ChatStream<'_> {
        let inner = self.inner.clone();
        let bucket = self.bucket.clone();

        Box::pin(async_stream::stream! {
            bucket.acquire().await;

            use futures::StreamExt;
            let mut stream = inner.stream_chat(request);
            while let Some(result) = stream.next().await {
                yield result;
            }
        })
    }
}
