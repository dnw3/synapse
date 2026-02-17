use std::sync::Arc;

use serde_json::json;
use synaptic_embeddings::{Embeddings, OpenAiEmbeddings, OpenAiEmbeddingsConfig};
use synaptic_models::backend::{FakeBackend, ProviderResponse};

#[tokio::test]
async fn openai_embed_query() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 200,
        body: json!({
            "data": [{"embedding": [0.1, 0.2, 0.3], "index": 0}],
            "usage": {"prompt_tokens": 5, "total_tokens": 5}
        }),
    });

    let config = OpenAiEmbeddingsConfig::new("test-key");
    let embeddings = OpenAiEmbeddings::new(config, backend);
    let result = embeddings.embed_query("hello").await.unwrap();

    assert_eq!(result.len(), 3);
    assert!((result[0] - 0.1).abs() < 0.001);
}

#[tokio::test]
async fn openai_embed_documents() {
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

    let config = OpenAiEmbeddingsConfig::new("test-key");
    let embeddings = OpenAiEmbeddings::new(config, backend);
    let results = embeddings
        .embed_documents(&["hello", "world"])
        .await
        .unwrap();

    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn openai_handles_api_error() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 429,
        body: json!({"error": {"message": "rate limited"}}),
    });

    let config = OpenAiEmbeddingsConfig::new("test-key");
    let embeddings = OpenAiEmbeddings::new(config, backend);
    let err = embeddings.embed_query("hello").await.unwrap_err();
    assert!(err.to_string().contains("429"));
}
