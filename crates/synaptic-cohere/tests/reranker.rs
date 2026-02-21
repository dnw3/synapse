use synaptic_cohere::{CohereReranker, CohereRerankerConfig};
use synaptic_core::Document;

#[test]
fn config_defaults() {
    let config = CohereRerankerConfig::new("test-key");
    assert_eq!(config.model, "rerank-v3.5");
    assert_eq!(config.api_key, "test-key");
    assert_eq!(config.base_url, "https://api.cohere.ai/v2");
    assert!(config.top_n.is_none());
}

#[test]
fn config_builder() {
    let config = CohereRerankerConfig::new("key")
        .with_model("rerank-english-v3.0")
        .with_top_n(5);
    assert_eq!(config.model, "rerank-english-v3.0");
    assert_eq!(config.top_n, Some(5));
}

#[test]
fn config_with_custom_base_url() {
    let config = CohereRerankerConfig::new("key").with_base_url("https://custom.example.com/v2");
    assert_eq!(config.base_url, "https://custom.example.com/v2");
}

#[tokio::test]
async fn rerank_empty_documents_returns_empty() {
    let config = CohereRerankerConfig::new("test-key");
    let reranker = CohereReranker::new(config);

    let result = reranker.rerank("query", Vec::new(), None).await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
#[ignore] // Requires Cohere API key
async fn integration_rerank() {
    let api_key = std::env::var("COHERE_API_KEY").expect("COHERE_API_KEY must be set");
    let config = CohereRerankerConfig::new(api_key).with_top_n(2);

    let reranker = CohereReranker::new(config);
    let docs = vec![
        Document::new("1", "Python is great for data science"),
        Document::new("2", "Rust is a systems programming language"),
        Document::new("3", "JavaScript runs in the browser"),
    ];

    let result = reranker
        .rerank("systems programming", docs, None)
        .await
        .unwrap();
    assert_eq!(result.len(), 2);

    // The Rust document should be ranked first.
    assert_eq!(result[0].id, "2");

    // Each document should have a relevance_score in metadata.
    for doc in &result {
        assert!(doc.metadata.contains_key("relevance_score"));
    }
}

#[tokio::test]
#[ignore] // Requires Cohere API key
async fn integration_rerank_top_n_override() {
    let api_key = std::env::var("COHERE_API_KEY").expect("COHERE_API_KEY must be set");
    let config = CohereRerankerConfig::new(api_key);

    let reranker = CohereReranker::new(config);
    let docs = vec![
        Document::new("1", "Apples are red"),
        Document::new("2", "Bananas are yellow"),
        Document::new("3", "Grapes are purple"),
        Document::new("4", "Oranges are orange"),
    ];

    // Override top_n at call site.
    let result = reranker.rerank("fruit colors", docs, Some(1)).await.unwrap();
    assert_eq!(result.len(), 1);
}
