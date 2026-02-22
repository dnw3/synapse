use synaptic_milvus::MilvusConfig;

#[test]
fn test_config_new() {
    let config = MilvusConfig::new("http://localhost:19530", "test_collection", 1536);
    assert_eq!(config.endpoint, "http://localhost:19530");
    assert_eq!(config.collection, "test_collection");
    assert_eq!(config.dim, 1536);
    assert!(config.api_key.is_none());
}

#[test]
fn test_config_with_api_key() {
    let config = MilvusConfig::new("http://localhost:19530", "test", 768).with_api_key("my-token");
    assert_eq!(config.api_key, Some("my-token".to_string()));
}

#[tokio::test]
#[ignore]
async fn test_integration_add_search() {
    // Requires live Milvus instance
    let config = MilvusConfig::new("http://localhost:19530", "test_coll", 4);
    let store = synaptic_milvus::MilvusVectorStore::new(config);
    store.initialize().await.unwrap();
}
