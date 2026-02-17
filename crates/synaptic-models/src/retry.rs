use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use synaptic_core::{ChatModel, ChatRequest, ChatResponse, ChatStream, SynapseError};

#[derive(Debug, Clone)]
pub struct RetryPolicy {
    pub max_attempts: usize,
    pub base_delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(500),
        }
    }
}

pub struct RetryChatModel {
    inner: Arc<dyn ChatModel>,
    policy: RetryPolicy,
}

impl RetryChatModel {
    pub fn new(inner: Arc<dyn ChatModel>, policy: RetryPolicy) -> Self {
        Self { inner, policy }
    }
}

fn is_retryable(err: &SynapseError) -> bool {
    matches!(err, SynapseError::RateLimit(_) | SynapseError::Timeout(_))
}

#[async_trait]
impl ChatModel for RetryChatModel {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, SynapseError> {
        let mut last_error = None;
        for attempt in 0..self.policy.max_attempts {
            match self.inner.chat(request.clone()).await {
                Ok(resp) => return Ok(resp),
                Err(e) if is_retryable(&e) && attempt + 1 < self.policy.max_attempts => {
                    let delay = self.policy.base_delay * 2u32.saturating_pow(attempt as u32);
                    tokio::time::sleep(delay).await;
                    last_error = Some(e);
                }
                Err(e) => return Err(e),
            }
        }
        Err(last_error.unwrap_or_else(|| SynapseError::Model("retry exhausted".to_string())))
    }

    fn stream_chat(&self, request: ChatRequest) -> ChatStream<'_> {
        let inner = self.inner.clone();
        let policy = self.policy.clone();

        Box::pin(async_stream::stream! {
            let mut last_error = None;
            for attempt in 0..policy.max_attempts {
                let mut stream = inner.stream_chat(request.clone());

                use futures::StreamExt;
                let mut chunks = Vec::new();
                let mut had_error = false;

                while let Some(result) = stream.next().await {
                    match result {
                        Ok(chunk) => chunks.push(chunk),
                        Err(e) if is_retryable(&e) && attempt + 1 < policy.max_attempts => {
                            let delay = policy.base_delay * 2u32.saturating_pow(attempt as u32);
                            tokio::time::sleep(delay).await;
                            last_error = Some(e);
                            had_error = true;
                            break;
                        }
                        Err(e) => {
                            yield Err(e);
                            return;
                        }
                    }
                }

                if !had_error {
                    for chunk in chunks {
                        yield Ok(chunk);
                    }
                    return;
                }
            }
            if let Some(e) = last_error {
                yield Err(e);
            }
        })
    }
}
