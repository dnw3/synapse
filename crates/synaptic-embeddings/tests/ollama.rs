use std::sync::Arc;

use serde_json::json;
use synaptic_embeddings::{Embeddings, OllamaEmbeddings, OllamaEmbeddingsConfig};
use synaptic_models::backend::{FakeBackend, ProviderResponse};

#[tokio::test]
async fn ollama_embed_query() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 200,
        body: json!({
            "model": "nomic-embed-text",
            "embeddings": [[0.5, 0.6, 0.7]]
        }),
    });

    let config = OllamaEmbeddingsConfig::new("nomic-embed-text");
    let embeddings = OllamaEmbeddings::new(config, backend);
    let result = embeddings.embed_query("hello").await.unwrap();

    assert_eq!(result.len(), 3);
    assert!((result[0] - 0.5).abs() < 0.001);
}

#[tokio::test]
async fn ollama_embed_documents() {
    let backend = Arc::new(FakeBackend::new());
    // Ollama processes one at a time
    backend.push_response(ProviderResponse {
        status: 200,
        body: json!({"embeddings": [[0.1, 0.2]]}),
    });
    backend.push_response(ProviderResponse {
        status: 200,
        body: json!({"embeddings": [[0.3, 0.4]]}),
    });

    let config = OllamaEmbeddingsConfig::new("nomic-embed-text");
    let embeddings = OllamaEmbeddings::new(config, backend);
    let results = embeddings
        .embed_documents(&["hello", "world"])
        .await
        .unwrap();

    assert_eq!(results.len(), 2);
}

#[tokio::test]
async fn ollama_handles_error() {
    let backend = Arc::new(FakeBackend::new());
    backend.push_response(ProviderResponse {
        status: 500,
        body: json!({"error": "model not found"}),
    });

    let config = OllamaEmbeddingsConfig::new("missing-model");
    let embeddings = OllamaEmbeddings::new(config, backend);
    let err = embeddings.embed_query("hello").await.unwrap_err();
    assert!(err.to_string().contains("500"));
}
