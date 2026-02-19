use std::sync::Arc;

use synaptic_cache::{CachedChatModel, InMemoryCache, LlmCache};
use synaptic_core::{ChatModel, ChatRequest, ChatResponse, Message};
use synaptic_models::ScriptedChatModel;

fn make_response(text: &str) -> ChatResponse {
    ChatResponse {
        message: Message::ai(text),
        usage: None,
    }
}

#[tokio::test]
async fn second_call_not_forwarded() {
    let scripted = Arc::new(ScriptedChatModel::new(vec![make_response("first only")]));
    let cache = Arc::new(InMemoryCache::new());
    let model = CachedChatModel::new(scripted, cache);

    let request = ChatRequest::new(vec![Message::human("test")]);

    // First call
    let r1 = model.chat(request.clone()).await.unwrap();
    assert_eq!(r1.message.content(), "first only");

    // Second call - should use cache, not model (which is now exhausted)
    let r2 = model.chat(request).await.unwrap();
    assert_eq!(r2.message.content(), "first only");
}

#[tokio::test]
async fn different_inputs_both_forwarded() {
    let scripted = Arc::new(ScriptedChatModel::new(vec![
        make_response("answer A"),
        make_response("answer B"),
    ]));
    let cache = Arc::new(InMemoryCache::new());
    let model = CachedChatModel::new(scripted, cache);

    let req_a = ChatRequest::new(vec![Message::human("question A")]);
    let req_b = ChatRequest::new(vec![Message::human("question B")]);

    let r1 = model.chat(req_a).await.unwrap();
    let r2 = model.chat(req_b).await.unwrap();

    assert_eq!(r1.message.content(), "answer A");
    assert_eq!(r2.message.content(), "answer B");
}

#[tokio::test]
async fn error_not_cached() {
    // ScriptedChatModel with no responses will error
    let scripted = Arc::new(ScriptedChatModel::new(vec![]));
    let cache = Arc::new(InMemoryCache::new());
    let model = CachedChatModel::new(scripted, cache);

    let request = ChatRequest::new(vec![Message::human("will fail")]);
    let err = model.chat(request).await.unwrap_err();
    assert!(err.to_string().contains("exhausted"));
}

#[tokio::test]
async fn cache_with_multiple_messages_in_request() {
    let scripted = Arc::new(ScriptedChatModel::new(vec![make_response("context answer")]));
    let cache = Arc::new(InMemoryCache::new());
    let model = CachedChatModel::new(scripted, cache);

    let request = ChatRequest::new(vec![
        Message::system("You are a helpful assistant"),
        Message::human("What is Rust?"),
    ]);

    let r1 = model.chat(request.clone()).await.unwrap();
    assert_eq!(r1.message.content(), "context answer");

    // Same multi-message request should hit cache
    let r2 = model.chat(request).await.unwrap();
    assert_eq!(r2.message.content(), "context answer");
}

#[tokio::test]
async fn cache_miss_after_clear() {
    let scripted = Arc::new(ScriptedChatModel::new(vec![
        make_response("before clear"),
        make_response("after clear"),
    ]));
    let cache = Arc::new(InMemoryCache::new());
    let model = CachedChatModel::new(scripted.clone(), cache.clone());

    let request = ChatRequest::new(vec![Message::human("test")]);

    let r1 = model.chat(request.clone()).await.unwrap();
    assert_eq!(r1.message.content(), "before clear");

    // Clear cache
    cache.clear().await.unwrap();

    // Should miss and call model again
    let r2 = model.chat(request).await.unwrap();
    assert_eq!(r2.message.content(), "after clear");
}
