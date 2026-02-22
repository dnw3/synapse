use std::sync::Arc;
use synaptic_models::{FakeBackend, ProviderResponse};
use synaptic_xai::{ChatModel, ChatRequest, Message, XaiChatModel, XaiConfig, XaiModel};

fn openai_chat_body(content: &str) -> serde_json::Value {
    serde_json::json!({
        "id": "chatcmpl-test",
        "choices": [{"message": {"role": "assistant", "content": content}, "finish_reason": "stop"}],
        "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15}
    })
}

#[tokio::test]
async fn test_basic_chat() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 200,
        body: openai_chat_body("Hello from Grok!"),
    });
    let config = XaiConfig::new("xai-test-key", XaiModel::Grok2Latest);
    let model = XaiChatModel::new(config, backend);
    let request = ChatRequest::new(vec![Message::human("Hi!")]);
    let response = model.chat(request).await.unwrap();
    assert_eq!(response.message.content(), "Hello from Grok!");
}

#[tokio::test]
async fn test_rate_limit_error() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 429,
        body: serde_json::json!({"error": {"message": "rate limited"}}),
    });
    let config = XaiConfig::new("xai-test-key", XaiModel::Grok2Mini);
    let model = XaiChatModel::new(config, backend);
    let request = ChatRequest::new(vec![Message::human("Hi!")]);
    let err = model.chat(request).await.unwrap_err();
    assert!(matches!(err, synaptic_xai::SynapticError::RateLimit(_)));
}

#[tokio::test]
async fn test_grok_beta() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 200,
        body: openai_chat_body("Legacy Grok response"),
    });
    let config = XaiConfig::new("xai-test-key", XaiModel::GrokBeta);
    let model = XaiChatModel::new(config, backend);
    let request = ChatRequest::new(vec![Message::human("Hello!")]);
    let response = model.chat(request).await.unwrap();
    assert_eq!(response.message.content(), "Legacy Grok response");
}
