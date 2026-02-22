use std::sync::Arc;
use synaptic_fireworks::{
    ChatModel, ChatRequest, FireworksChatModel, FireworksConfig, FireworksModel, Message,
};
use synaptic_models::{FakeBackend, ProviderResponse};

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
        body: openai_chat_body("Hello from Fireworks!"),
    });
    let config = FireworksConfig::new("fw-test-key", FireworksModel::Llama3_1_70bInstruct);
    let model = FireworksChatModel::new(config, backend);
    let request = ChatRequest::new(vec![Message::human("Hi!")]);
    let response = model.chat(request).await.unwrap();
    assert_eq!(response.message.content(), "Hello from Fireworks!");
}

#[tokio::test]
async fn test_rate_limit_error() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 429,
        body: serde_json::json!({"error": {"message": "rate limited"}}),
    });
    let config = FireworksConfig::new("fw-test-key", FireworksModel::Llama3_1_8bInstruct);
    let model = FireworksChatModel::new(config, backend);
    let request = ChatRequest::new(vec![Message::human("Hi!")]);
    let err = model.chat(request).await.unwrap_err();
    assert!(matches!(
        err,
        synaptic_fireworks::SynapticError::RateLimit(_)
    ));
}

#[tokio::test]
async fn test_system_message() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 200,
        body: openai_chat_body("I am a helpful assistant."),
    });
    let config = FireworksConfig::new("fw-test-key", FireworksModel::DeepSeekR1);
    let model = FireworksChatModel::new(config, backend);
    let request = ChatRequest::new(vec![
        Message::system("You are a helpful assistant."),
        Message::human("Who are you?"),
    ]);
    let response = model.chat(request).await.unwrap();
    assert_eq!(response.message.content(), "I am a helpful assistant.");
}
