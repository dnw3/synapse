use std::sync::Arc;
use synaptic_models::{FakeBackend, ProviderResponse};
use synaptic_perplexity::{
    ChatModel, ChatRequest, Message, PerplexityChatModel, PerplexityConfig, PerplexityModel,
};

fn openai_chat_body(content: &str) -> serde_json::Value {
    serde_json::json!({
        "id": "chatcmpl-test",
        "choices": [{"message": {"role": "assistant", "content": content}, "finish_reason": "stop"}],
        "usage": {"prompt_tokens": 10, "completion_tokens": 20, "total_tokens": 30},
        "citations": ["https://example.com/source1", "https://example.com/source2"]
    })
}

#[tokio::test]
async fn test_basic_chat() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 200,
        body: openai_chat_body("According to recent sources, Rust is memory-safe."),
    });
    let config = PerplexityConfig::new("pplx-test-key", PerplexityModel::SonarLarge);
    let model = PerplexityChatModel::new(config, backend);
    let request = ChatRequest::new(vec![Message::human("What is Rust?")]);
    let response = model.chat(request).await.unwrap();
    assert_eq!(
        response.message.content(),
        "According to recent sources, Rust is memory-safe."
    );
}

#[tokio::test]
async fn test_rate_limit_error() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 429,
        body: serde_json::json!({"error": {"message": "rate limited"}}),
    });
    let config = PerplexityConfig::new("pplx-test-key", PerplexityModel::SonarSmall);
    let model = PerplexityChatModel::new(config, backend);
    let request = ChatRequest::new(vec![Message::human("Hi!")]);
    let err = model.chat(request).await.unwrap_err();
    assert!(matches!(
        err,
        synaptic_perplexity::SynapticError::RateLimit(_)
    ));
}

#[tokio::test]
async fn test_sonar_reasoning() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 200,
        body: openai_chat_body("Let me reason step by step..."),
    });
    let config = PerplexityConfig::new("pplx-test-key", PerplexityModel::SonarReasoningPro)
        .with_temperature(0.2);
    let model = PerplexityChatModel::new(config, backend);
    let request = ChatRequest::new(vec![Message::human("Solve: 2+2")]);
    let response = model.chat(request).await.unwrap();
    assert_eq!(response.message.content(), "Let me reason step by step...");
}
