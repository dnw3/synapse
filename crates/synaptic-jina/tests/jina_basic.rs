use synaptic_jina::{JinaConfig, JinaEmbeddingModel, JinaEmbeddings};

#[test]
fn test_model_as_str() {
    assert_eq!(
        JinaEmbeddingModel::JinaEmbeddingsV3.as_str(),
        "jina-embeddings-v3"
    );
    assert_eq!(
        JinaEmbeddingModel::JinaEmbeddingsV2BaseEn.as_str(),
        "jina-embeddings-v2-base-en"
    );
    assert_eq!(JinaEmbeddingModel::JinaClipV2.as_str(), "jina-clip-v2");
    assert_eq!(
        JinaEmbeddingModel::Custom("jina-v4".into()).as_str(),
        "jina-v4"
    );
}

#[test]
fn test_config_builder() {
    let config = JinaConfig::new("test-key", JinaEmbeddingModel::JinaEmbeddingsV3);
    assert_eq!(config.model, "jina-embeddings-v3");
    assert_eq!(config.base_url, "https://api.jina.ai/v1");
}

#[test]
fn test_embeddings_new() {
    let config = JinaConfig::new("test-key", JinaEmbeddingModel::JinaEmbeddingsV3);
    let _embeddings = JinaEmbeddings::new(config);
}

#[tokio::test]
#[ignore]
async fn test_embed_documents_integration() {
    let api_key = std::env::var("JINA_API_KEY").unwrap();
    let config = JinaConfig::new(api_key, JinaEmbeddingModel::JinaEmbeddingsV3);
    let embeddings = JinaEmbeddings::new(config);
    use synaptic_core::Embeddings;
    let result = embeddings.embed_documents(&["Hello world"]).await.unwrap();
    assert_eq!(result.len(), 1);
}
