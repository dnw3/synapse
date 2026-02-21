use std::sync::Arc;

use serde_json::json;
use synaptic_core::{ChatModel, ChatRequest, Embeddings, Message};
use synaptic_models::{FakeBackend, ProviderResponse};
use synaptic_openai::{
    AzureOpenAiChatModel, AzureOpenAiConfig, AzureOpenAiEmbeddings, AzureOpenAiEmbeddingsConfig,
};

// ---------------------------------------------------------------------------
// AzureOpenAiConfig
// ---------------------------------------------------------------------------

#[test]
fn config_defaults() {
    let config = AzureOpenAiConfig::new("key", "my-resource", "gpt-4");
    assert_eq!(config.api_key, "key");
    assert_eq!(config.resource_name, "my-resource");
    assert_eq!(config.deployment_name, "gpt-4");
    assert_eq!(config.api_version, "2024-10-21");
    assert!(config.max_tokens.is_none());
    assert!(config.temperature.is_none());
}

#[test]
fn config_builder_methods() {
    let config = AzureOpenAiConfig::new("key", "res", "dep")
        .with_api_version("2025-01-01")
        .with_max_tokens(500)
        .with_temperature(0.5)
        .with_top_p(0.9)
        .with_stop(vec!["END".to_string()]);

    assert_eq!(config.api_version, "2025-01-01");
    assert_eq!(config.max_tokens, Some(500));
    assert_eq!(config.temperature, Some(0.5));
    assert_eq!(config.top_p, Some(0.9));
    assert_eq!(config.stop, Some(vec!["END".to_string()]));
}

// ---------------------------------------------------------------------------
// build_request verification
// ---------------------------------------------------------------------------

#[test]
fn build_request_uses_azure_url_and_api_key_header() {
    let backend = Arc::new(FakeBackend::new());
    let config = AzureOpenAiConfig::new("test-key", "my-resource", "gpt-4");
    let model = AzureOpenAiChatModel::new(config, backend);

    let request = ChatRequest::new(vec![Message::human("hello")]);
    let provider_req = model.build_request(&request, false);

    // URL pattern
    assert!(
        provider_req
            .url
            .contains("my-resource.openai.azure.com"),
        "URL should contain resource name: {}",
        provider_req.url
    );
    assert!(
        provider_req.url.contains("deployments/gpt-4"),
        "URL should contain deployment name: {}",
        provider_req.url
    );
    assert!(
        provider_req.url.contains("api-version=2024-10-21"),
        "URL should contain api version: {}",
        provider_req.url
    );
    assert!(
        provider_req.url.contains("chat/completions"),
        "URL should end with chat/completions: {}",
        provider_req.url
    );

    // api-key header (not Bearer)
    let has_api_key = provider_req
        .headers
        .iter()
        .any(|(k, v)| k == "api-key" && v == "test-key");
    assert!(has_api_key, "should have api-key header");

    let has_bearer = provider_req
        .headers
        .iter()
        .any(|(k, _)| k == "Authorization");
    assert!(!has_bearer, "should NOT have Authorization header");
}

#[test]
fn build_request_does_not_include_model_in_body() {
    let backend = Arc::new(FakeBackend::new());
    let config = AzureOpenAiConfig::new("key", "res", "dep");
    let model = AzureOpenAiChatModel::new(config, backend);

    let request = ChatRequest::new(vec![Message::human("hi")]);
    let provider_req = model.build_request(&request, false);

    // Azure deployment URL already specifies the model; body should not.
    assert!(
        provider_req.body.get("model").is_none(),
        "Azure body should not contain 'model' field"
    );
}

// ---------------------------------------------------------------------------
// Chat round-trip via FakeBackend
// ---------------------------------------------------------------------------

#[tokio::test]
async fn azure_chat_parses_response() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 200,
        body: json!({
            "choices": [{"message": {"role": "assistant", "content": "hello from azure"}}],
            "usage": {"prompt_tokens": 5, "completion_tokens": 3, "total_tokens": 8}
        }),
    });

    let config = AzureOpenAiConfig::new("test-key", "my-resource", "gpt-4");
    let model = AzureOpenAiChatModel::new(config, backend);
    let response = model
        .chat(ChatRequest::new(vec![Message::human("hi")]))
        .await
        .unwrap();

    assert_eq!(response.message.content(), "hello from azure");
    let usage = response.usage.unwrap();
    assert_eq!(usage.input_tokens, 5);
    assert_eq!(usage.output_tokens, 3);
    assert_eq!(usage.total_tokens, 8);
}

#[tokio::test]
async fn azure_chat_handles_rate_limit() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 429,
        body: json!({"error": {"message": "too many requests"}}),
    });

    let config = AzureOpenAiConfig::new("key", "res", "dep");
    let model = AzureOpenAiChatModel::new(config, backend);
    let err = model
        .chat(ChatRequest::new(vec![Message::human("hi")]))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("rate limit"));
}

#[tokio::test]
async fn azure_chat_handles_api_error() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 500,
        body: json!({"error": {"message": "internal server error"}}),
    });

    let config = AzureOpenAiConfig::new("key", "res", "dep");
    let model = AzureOpenAiChatModel::new(config, backend);
    let err = model
        .chat(ChatRequest::new(vec![Message::human("hi")]))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("API error"));
}

// ---------------------------------------------------------------------------
// Azure Embeddings
// ---------------------------------------------------------------------------

#[test]
fn embeddings_config_defaults() {
    let config = AzureOpenAiEmbeddingsConfig::new("key", "res", "emb-dep");
    assert_eq!(config.api_key, "key");
    assert_eq!(config.resource_name, "res");
    assert_eq!(config.deployment_name, "emb-dep");
    assert_eq!(config.api_version, "2024-10-21");
    assert_eq!(config.model, "text-embedding-3-small");
}

#[tokio::test]
async fn azure_embed_query() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 200,
        body: json!({
            "data": [{"embedding": [0.1, 0.2, 0.3], "index": 0}],
            "usage": {"prompt_tokens": 5, "total_tokens": 5}
        }),
    });

    let config = AzureOpenAiEmbeddingsConfig::new("key", "res", "emb-dep");
    let embeddings = AzureOpenAiEmbeddings::new(config, backend);
    let result = embeddings.embed_query("hello").await.unwrap();

    assert_eq!(result.len(), 3);
    assert!((result[0] - 0.1).abs() < 0.001);
}

#[tokio::test]
async fn azure_embed_documents() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 200,
        body: json!({
            "data": [
                {"embedding": [0.1, 0.2], "index": 0},
                {"embedding": [0.3, 0.4], "index": 1}
            ]
        }),
    });

    let config = AzureOpenAiEmbeddingsConfig::new("key", "res", "emb-dep");
    let embeddings = AzureOpenAiEmbeddings::new(config, backend);
    let results = embeddings
        .embed_documents(&["hello", "world"])
        .await
        .unwrap();

    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn azure_embeddings_handles_error() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 429,
        body: json!({"error": {"message": "rate limited"}}),
    });

    let config = AzureOpenAiEmbeddingsConfig::new("key", "res", "emb-dep");
    let embeddings = AzureOpenAiEmbeddings::new(config, backend);
    let err = embeddings.embed_query("hello").await.unwrap_err();
    assert!(err.to_string().contains("429"));
}
