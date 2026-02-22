use synaptic_core::Document;
use synaptic_voyage::reranker::{VoyageReranker, VoyageRerankerModel};

#[test]
fn model_as_str() {
    assert_eq!(VoyageRerankerModel::Rerank2.as_str(), "rerank-2");
    assert_eq!(VoyageRerankerModel::Rerank2Lite.as_str(), "rerank-2-lite");
    assert_eq!(
        VoyageRerankerModel::Custom("my-model".into()).as_str(),
        "my-model"
    );
}

#[test]
fn model_display() {
    assert_eq!(VoyageRerankerModel::Rerank2.to_string(), "rerank-2");
}

#[test]
fn reranker_builder() {
    let r = VoyageReranker::new("pa-key")
        .with_model(VoyageRerankerModel::Rerank2Lite)
        .with_base_url("https://custom.voyage.ai/v1");
    let _ = r;
}

#[tokio::test]
#[ignore]
async fn rerank_integration() {
    let api_key = std::env::var("VOYAGE_API_KEY").unwrap();
    let reranker = VoyageReranker::new(api_key);
    let docs = vec![
        Document::new("doc1", "Paris is the capital of France."),
        Document::new("doc2", "Berlin is the capital of Germany."),
    ];
    let results = reranker.rerank("capital of France", docs, 1).await.unwrap();
    assert_eq!(results.len(), 1);
}
