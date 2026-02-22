use synaptic_nomic::{NomicConfig, NomicEmbeddings, NomicModel, NomicTaskType};

#[test]
fn test_model_as_str() {
    assert_eq!(
        NomicModel::NomicEmbedTextV1_5.as_str(),
        "nomic-embed-text-v1.5"
    );
    assert_eq!(NomicModel::NomicEmbedTextV1.as_str(), "nomic-embed-text-v1");
    assert_eq!(NomicModel::Custom("nomic-v3".into()).as_str(), "nomic-v3");
}

#[test]
fn test_task_type_as_str() {
    assert_eq!(NomicTaskType::SearchDocument.as_str(), "search_document");
    assert_eq!(NomicTaskType::SearchQuery.as_str(), "search_query");
    assert_eq!(NomicTaskType::Classification.as_str(), "classification");
    assert_eq!(NomicTaskType::Clustering.as_str(), "clustering");
}

#[test]
fn test_config_defaults() {
    let config = NomicConfig::new("test-key");
    assert_eq!(config.model, "nomic-embed-text-v1.5");
    assert_eq!(config.base_url, "https://api-atlas.nomic.ai/v1");
}

#[test]
fn test_config_with_model() {
    let config = NomicConfig::new("key").with_model(NomicModel::NomicEmbedTextV1);
    assert_eq!(config.model, "nomic-embed-text-v1");
}

#[test]
fn test_embeddings_new() {
    let config = NomicConfig::new("test-key");
    let _embeddings = NomicEmbeddings::new(config);
}

#[tokio::test]
#[ignore]
async fn test_embed_documents_integration() {
    let api_key = std::env::var("NOMIC_API_KEY").unwrap();
    let config = NomicConfig::new(api_key);
    let embeddings = NomicEmbeddings::new(config);
    use synaptic_core::Embeddings;
    let result = embeddings.embed_documents(&["Hello world"]).await.unwrap();
    assert_eq!(result.len(), 1);
    assert!(!result[0].is_empty());
}
