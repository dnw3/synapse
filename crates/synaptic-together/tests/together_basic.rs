use std::sync::Arc;
use synaptic_models::{FakeBackend, ProviderResponse};
use synaptic_together::{
    ChatModel, ChatRequest, Message, TogetherChatModel, TogetherConfig, TogetherModel,
};

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
        body: openai_chat_body("Hello from Together AI!"),
    });
    let config = TogetherConfig::new("test-key", TogetherModel::Llama3_3_70bInstructTurbo);
    let model = TogetherChatModel::new(config, backend);
    let request = ChatRequest::new(vec![Message::human("Hi!")]);
    let response = model.chat(request).await.unwrap();
    assert_eq!(response.message.content(), "Hello from Together AI!");
}

#[tokio::test]
async fn test_rate_limit_error() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 429,
        body: serde_json::json!({"error": {"message": "rate limited"}}),
    });
    let config = TogetherConfig::new("test-key", TogetherModel::Llama3_1_8bInstructTurbo);
    let model = TogetherChatModel::new(config, backend);
    let request = ChatRequest::new(vec![Message::human("Hi!")]);
    let err = model.chat(request).await.unwrap_err();
    assert!(matches!(
        err,
        synaptic_together::SynapticError::RateLimit(_)
    ));
}

#[tokio::test]
async fn test_custom_model() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 200,
        body: openai_chat_body("Custom model response"),
    });
    let config = TogetherConfig::new_custom("test-key", "custom/model-v1");
    let model = TogetherChatModel::new(config, backend);
    let request = ChatRequest::new(vec![
        Message::system("You are helpful."),
        Message::human("Hello!"),
    ]);
    let response = model.chat(request).await.unwrap();
    assert_eq!(response.message.content(), "Custom model response");
}
