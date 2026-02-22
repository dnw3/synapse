use synaptic_core::Document;
use synaptic_huggingface::reranker::{BgeRerankerModel, HuggingFaceReranker};

#[test]
fn model_as_str() {
    assert_eq!(
        BgeRerankerModel::BgeRerankerV2M3.as_str(),
        "BAAI/bge-reranker-v2-m3"
    );
    assert_eq!(
        BgeRerankerModel::BgeRerankerLarge.as_str(),
        "BAAI/bge-reranker-large"
    );
    assert_eq!(
        BgeRerankerModel::BgeRerankerBase.as_str(),
        "BAAI/bge-reranker-base"
    );
    assert_eq!(
        BgeRerankerModel::Custom("my/model".into()).as_str(),
        "my/model"
    );
}

#[test]
fn model_display() {
    assert_eq!(
        BgeRerankerModel::BgeRerankerV2M3.to_string(),
        "BAAI/bge-reranker-v2-m3"
    );
}

#[test]
fn reranker_builder() {
    let r = HuggingFaceReranker::new("hf_test")
        .with_model(BgeRerankerModel::BgeRerankerLarge)
        .with_base_url("https://custom.hf.co/models");
    let _ = r;
}

#[tokio::test]
#[ignore]
async fn rerank_integration() {
    let api_key = std::env::var("HF_API_KEY").unwrap();
    let reranker = HuggingFaceReranker::new(api_key);
    let docs = vec![
        Document::new("doc1", "Paris is the capital of France."),
        Document::new("doc2", "Berlin is the capital of Germany."),
        Document::new("doc3", "The Eiffel Tower is in Paris."),
    ];
    let results = reranker
        .rerank("What is the capital of France?", docs, 2)
        .await
        .unwrap();
    assert_eq!(results.len(), 2);
    assert!(results[0].1 >= results[1].1);
}
