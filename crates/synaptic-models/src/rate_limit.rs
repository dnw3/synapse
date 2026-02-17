use std::sync::Arc;

use async_trait::async_trait;
use synaptic_core::{ChatModel, ChatRequest, ChatResponse, ChatStream, SynapseError};
use tokio::sync::Semaphore;

pub struct RateLimitedChatModel {
    inner: Arc<dyn ChatModel>,
    semaphore: Arc<Semaphore>,
}

impl RateLimitedChatModel {
    pub fn new(inner: Arc<dyn ChatModel>, max_concurrent: usize) -> Self {
        Self {
            inner,
            semaphore: Arc::new(Semaphore::new(max_concurrent)),
        }
    }
}

#[async_trait]
impl ChatModel for RateLimitedChatModel {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, SynapseError> {
        let _permit = self
            .semaphore
            .acquire()
            .await
            .map_err(|e| SynapseError::Model(format!("semaphore error: {e}")))?;
        self.inner.chat(request).await
    }

    fn stream_chat(&self, request: ChatRequest) -> ChatStream<'_> {
        let inner = self.inner.clone();
        let semaphore = self.semaphore.clone();

        Box::pin(async_stream::stream! {
            let _permit = match semaphore.acquire_owned().await {
                Ok(p) => p,
                Err(e) => {
                    yield Err(SynapseError::Model(format!("semaphore error: {e}")));
                    return;
                }
            };

            use futures::StreamExt;
            let mut stream = inner.stream_chat(request);
            while let Some(result) = stream.next().await {
                yield result;
            }
        })
    }
}
