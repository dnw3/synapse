use std::sync::Arc;

use synapse_cache::{CachedChatModel, InMemoryCache};
use synapse_core::{ChatModel, ChatRequest, ChatResponse, Message};
use synapse_models::ScriptedChatModel;

fn make_response(text: &str) -> ChatResponse {
    ChatResponse {
        message: Message::ai(text),
        usage: None,
    }
}

#[tokio::test]
async fn cached_model_returns_cached_on_hit() {
    let scripted = Arc::new(ScriptedChatModel::new(vec![make_response("first call")]));
    let cache = Arc::new(InMemoryCache::new());
    let model = CachedChatModel::new(scripted, cache);

    let request = ChatRequest::new(vec![Message::human("hello")]);

    // First call should go to the inner model
    let r1 = model.chat(request.clone()).await.unwrap();
    assert_eq!(r1.message.content(), "first call");

    // Second call with same request should return cached response
    // (ScriptedChatModel would error if called again since it only had one response)
    let r2 = model.chat(request).await.unwrap();
    assert_eq!(r2.message.content(), "first call");
}

#[tokio::test]
async fn cached_model_calls_model_on_miss() {
    let scripted = Arc::new(ScriptedChatModel::new(vec![make_response("response")]));
    let cache = Arc::new(InMemoryCache::new());
    let model = CachedChatModel::new(scripted, cache);

    let request = ChatRequest::new(vec![Message::human("hello")]);
    let result = model.chat(request).await.unwrap();
    assert_eq!(result.message.content(), "response");
}

#[tokio::test]
async fn cached_model_different_requests_not_cached() {
    let scripted = Arc::new(ScriptedChatModel::new(vec![
        make_response("answer A"),
        make_response("answer B"),
    ]));
    let cache = Arc::new(InMemoryCache::new());
    let model = CachedChatModel::new(scripted, cache);

    let req_a = ChatRequest::new(vec![Message::human("question A")]);
    let req_b = ChatRequest::new(vec![Message::human("question B")]);

    let r1 = model.chat(req_a).await.unwrap();
    assert_eq!(r1.message.content(), "answer A");

    let r2 = model.chat(req_b).await.unwrap();
    assert_eq!(r2.message.content(), "answer B");
}
