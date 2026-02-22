use synaptic_core::Embeddings;
use synaptic_voyage::{VoyageConfig, VoyageEmbeddings, VoyageModel};

#[test]
fn test_model_as_str() {
    assert_eq!(VoyageModel::Voyage3Large.as_str(), "voyage-3-large");
    assert_eq!(VoyageModel::Voyage3.as_str(), "voyage-3");
    assert_eq!(VoyageModel::Voyage3Lite.as_str(), "voyage-3-lite");
    assert_eq!(VoyageModel::VoyageCode3.as_str(), "voyage-code-3");
    assert_eq!(VoyageModel::VoyageFinance2.as_str(), "voyage-finance-2");
    assert_eq!(
        VoyageModel::Custom("voyage-v2".into()).as_str(),
        "voyage-v2"
    );
}

#[test]
fn test_config_builder() {
    let config = VoyageConfig::new("key", VoyageModel::Voyage3Large).with_input_type("document");
    assert_eq!(config.model, "voyage-3-large");
    assert_eq!(config.base_url, "https://api.voyageai.com/v1");
    assert_eq!(config.input_type, Some("document".to_string()));
}

#[test]
fn test_model_display() {
    assert_eq!(format!("{}", VoyageModel::Voyage3), "voyage-3");
}

#[test]
fn test_embeddings_new() {
    let config = VoyageConfig::new("test-key", VoyageModel::Voyage3Large);
    let _embeddings = VoyageEmbeddings::new(config);
    // Just verify construction works
}

// Integration test requiring real API key
#[tokio::test]
#[ignore]
async fn test_embed_documents_integration() {
    let api_key = std::env::var("VOYAGE_API_KEY").unwrap();
    let config = VoyageConfig::new(api_key, VoyageModel::Voyage3);
    let embeddings = VoyageEmbeddings::new(config);
    let result = embeddings
        .embed_documents(&["Hello world", "Rust is fast"])
        .await
        .unwrap();
    assert_eq!(result.len(), 2);
    assert!(!result[0].is_empty());
}
