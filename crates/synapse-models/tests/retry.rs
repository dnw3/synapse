use std::sync::Arc;
use std::time::Duration;

use futures::StreamExt;
use synapse_core::{ChatModel, ChatRequest, ChatResponse, ChatStream, Message, SynapseError};
use synapse_models::{RetryChatModel, RetryPolicy};
use tokio::sync::Mutex;

struct FailThenSucceedModel {
    attempts: Arc<Mutex<usize>>,
    fail_count: usize,
    error_kind: &'static str,
}

impl FailThenSucceedModel {
    fn new(fail_count: usize, error_kind: &'static str) -> Self {
        Self {
            attempts: Arc::new(Mutex::new(0)),
            fail_count,
            error_kind,
        }
    }
}

#[async_trait::async_trait]
impl ChatModel for FailThenSucceedModel {
    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, SynapseError> {
        let mut attempts = self.attempts.lock().await;
        *attempts += 1;
        if *attempts <= self.fail_count {
            match self.error_kind {
                "rate_limit" => Err(SynapseError::RateLimit("rate limited".to_string())),
                "timeout" => Err(SynapseError::Timeout("timed out".to_string())),
                _ => Err(SynapseError::Model("non-retryable".to_string())),
            }
        } else {
            Ok(ChatResponse {
                message: Message::ai("success"),
                usage: None,
            })
        }
    }
}

#[tokio::test]
async fn retries_on_rate_limit() {
    let inner = Arc::new(FailThenSucceedModel::new(2, "rate_limit"));
    let policy = RetryPolicy {
        max_attempts: 3,
        base_delay: Duration::from_millis(1),
    };
    let model = RetryChatModel::new(inner.clone(), policy);
    let request = ChatRequest::new(vec![Message::human("hi")]);
    let response = model.chat(request).await.unwrap();
    assert_eq!(response.message.content(), "success");
    assert_eq!(*inner.attempts.lock().await, 3);
}

#[tokio::test]
async fn retries_on_timeout() {
    let inner = Arc::new(FailThenSucceedModel::new(1, "timeout"));
    let policy = RetryPolicy {
        max_attempts: 3,
        base_delay: Duration::from_millis(1),
    };
    let model = RetryChatModel::new(inner.clone(), policy);
    let request = ChatRequest::new(vec![Message::human("hi")]);
    let response = model.chat(request).await.unwrap();
    assert_eq!(response.message.content(), "success");
    assert_eq!(*inner.attempts.lock().await, 2);
}

#[tokio::test]
async fn does_not_retry_non_retryable_error() {
    let inner = Arc::new(FailThenSucceedModel::new(1, "model"));
    let policy = RetryPolicy {
        max_attempts: 3,
        base_delay: Duration::from_millis(1),
    };
    let model = RetryChatModel::new(inner.clone(), policy);
    let request = ChatRequest::new(vec![Message::human("hi")]);
    let err = model.chat(request).await.unwrap_err();
    assert!(err.to_string().contains("non-retryable"));
    assert_eq!(*inner.attempts.lock().await, 1);
}

#[tokio::test]
async fn exhausts_retries() {
    let inner = Arc::new(FailThenSucceedModel::new(5, "rate_limit"));
    let policy = RetryPolicy {
        max_attempts: 3,
        base_delay: Duration::from_millis(1),
    };
    let model = RetryChatModel::new(inner.clone(), policy);
    let request = ChatRequest::new(vec![Message::human("hi")]);
    let err = model.chat(request).await.unwrap_err();
    assert!(err.to_string().contains("rate limit"));
    assert_eq!(*inner.attempts.lock().await, 3);
}

struct StreamOnceModel;

#[async_trait::async_trait]
impl ChatModel for StreamOnceModel {
    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, SynapseError> {
        Ok(ChatResponse {
            message: Message::ai("streamed"),
            usage: None,
        })
    }

    fn stream_chat(&self, _request: ChatRequest) -> ChatStream<'_> {
        Box::pin(async_stream::stream! {
            yield Ok(synapse_core::AIMessageChunk {
                content: "chunk".to_string(),
                ..Default::default()
            });
        })
    }
}

#[tokio::test]
async fn retry_stream_chat_succeeds() {
    let inner = Arc::new(StreamOnceModel);
    let policy = RetryPolicy {
        max_attempts: 2,
        base_delay: Duration::from_millis(1),
    };
    let model = RetryChatModel::new(inner, policy);
    let request = ChatRequest::new(vec![Message::human("hi")]);
    let chunks: Vec<_> = model
        .stream_chat(request)
        .collect::<Vec<_>>()
        .await
        .into_iter()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0].content, "chunk");
}
