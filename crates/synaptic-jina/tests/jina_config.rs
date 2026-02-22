use synaptic_jina::reranker::{JinaReranker, JinaRerankerModel};
use synaptic_jina::{JinaConfig, JinaEmbeddingModel};

#[test]
fn test_jina_config() {
    let config = JinaConfig::new("key", JinaEmbeddingModel::JinaEmbeddingsV3);
    assert_eq!(config.api_key, "key");
    assert_eq!(config.model, "jina-embeddings-v3");
}

#[test]
fn test_reranker_model() {
    assert_eq!(
        JinaRerankerModel::JinaRerankerV2BaseMultilingual.as_str(),
        "jina-reranker-v2-base-multilingual"
    );
    assert_eq!(
        JinaRerankerModel::JinaRerankerV1BaseEn.as_str(),
        "jina-reranker-v1-base-en"
    );
}

#[test]
fn test_reranker_new() {
    let _reranker = JinaReranker::new("test-key");
}

#[test]
fn test_model_display() {
    assert_eq!(
        format!("{}", JinaEmbeddingModel::JinaEmbeddingsV3),
        "jina-embeddings-v3"
    );
}
