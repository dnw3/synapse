use std::sync::Arc;

use synaptic_cache::{CachedChatModel, InMemoryCache};
use synaptic_core::{ChatModel, ChatRequest, ChatResponse, Message};
use synaptic_models::ScriptedChatModel;

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

#[tokio::test]
async fn cached_model_with_ttl() {
    use std::time::Duration;
    use synaptic_cache::InMemoryCache;

    let scripted = Arc::new(ScriptedChatModel::new(vec![
        make_response("first"),
        make_response("second"),
    ]));
    let cache = Arc::new(InMemoryCache::with_ttl(Duration::from_millis(50)));
    let model = CachedChatModel::new(scripted, cache);

    let request = ChatRequest::new(vec![Message::human("hello")]);

    let r1 = model.chat(request.clone()).await.unwrap();
    assert_eq!(r1.message.content(), "first");

    // Should be cached
    let r2 = model.chat(request.clone()).await.unwrap();
    assert_eq!(r2.message.content(), "first");

    // Wait for TTL to expire
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Now cache miss, should get second response
    let r3 = model.chat(request).await.unwrap();
    assert_eq!(r3.message.content(), "second");
}

#[tokio::test]
async fn cached_model_with_tools_in_request() {
    use synaptic_core::ToolDefinition;

    let scripted = Arc::new(ScriptedChatModel::new(vec![make_response("tool response")]));
    let cache = Arc::new(InMemoryCache::new());
    let model = CachedChatModel::new(scripted, cache);

    let request =
        ChatRequest::new(vec![Message::human("use tool")]).with_tools(vec![ToolDefinition {
            name: "test_tool".to_string(),
            description: "A test tool".to_string(),
            parameters: serde_json::json!({"type": "object"}),
            extras: None,
        }]);

    let r1 = model.chat(request.clone()).await.unwrap();
    assert_eq!(r1.message.content(), "tool response");

    // Same request with tools should hit cache
    let r2 = model.chat(request).await.unwrap();
    assert_eq!(r2.message.content(), "tool response");
}
