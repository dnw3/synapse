use std::sync::Arc;
use std::time::Instant;

use futures::StreamExt;
use synapse_core::{
    AIMessageChunk, ChatModel, ChatRequest, ChatResponse, ChatStream, Message, SynapseError,
};
use synapse_models::RateLimitedChatModel;
use tokio::sync::Mutex;

struct SlowModel {
    call_count: Arc<Mutex<usize>>,
}

impl SlowModel {
    fn new() -> Self {
        Self {
            call_count: Arc::new(Mutex::new(0)),
        }
    }
}

#[async_trait::async_trait]
impl ChatModel for SlowModel {
    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, SynapseError> {
        {
            let mut count = self.call_count.lock().await;
            *count += 1;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        Ok(ChatResponse {
            message: Message::ai("done"),
            usage: None,
        })
    }

    fn stream_chat(&self, _request: ChatRequest) -> ChatStream<'_> {
        Box::pin(async_stream::stream! {
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
            yield Ok(AIMessageChunk {
                content: "chunk".to_string(),
                ..Default::default()
            });
        })
    }
}

#[tokio::test]
async fn limits_concurrent_calls() {
    let inner = Arc::new(SlowModel::new());
    let model = Arc::new(RateLimitedChatModel::new(inner.clone(), 1));

    let start = Instant::now();

    let m1 = model.clone();
    let m2 = model.clone();

    let (r1, r2) = tokio::join!(
        async move { m1.chat(ChatRequest::new(vec![Message::human("a")])).await },
        async move { m2.chat(ChatRequest::new(vec![Message::human("b")])).await },
    );

    r1.unwrap();
    r2.unwrap();

    // With concurrency=1, two 50ms calls should take >= 100ms
    assert!(start.elapsed().as_millis() >= 90);
    assert_eq!(*inner.call_count.lock().await, 2);
}

#[tokio::test]
async fn allows_concurrent_up_to_limit() {
    let inner = Arc::new(SlowModel::new());
    let model = Arc::new(RateLimitedChatModel::new(inner.clone(), 2));

    let start = Instant::now();

    let m1 = model.clone();
    let m2 = model.clone();

    let (r1, r2) = tokio::join!(
        async move { m1.chat(ChatRequest::new(vec![Message::human("a")])).await },
        async move { m2.chat(ChatRequest::new(vec![Message::human("b")])).await },
    );

    r1.unwrap();
    r2.unwrap();

    // With concurrency=2, both should run in parallel, ~50ms total
    assert!(start.elapsed().as_millis() < 90);
    assert_eq!(*inner.call_count.lock().await, 2);
}

#[tokio::test]
async fn rate_limited_stream_chat() {
    let inner = Arc::new(SlowModel::new());
    let model = RateLimitedChatModel::new(inner, 1);

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
