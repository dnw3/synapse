use synaptic_opensearch::OpenSearchConfig;

#[test]
fn test_config_new() {
    let config = OpenSearchConfig::new("http://localhost:9200", "test_index", 1536);
    assert_eq!(config.endpoint, "http://localhost:9200");
    assert_eq!(config.index, "test_index");
    assert_eq!(config.dim, 1536);
}

#[test]
fn test_config_with_credentials() {
    let config = OpenSearchConfig::new("http://localhost:9200", "test", 768)
        .with_credentials("admin", "password");
    assert_eq!(config.username, Some("admin".to_string()));
    assert_eq!(config.password, Some("password".to_string()));
}

#[tokio::test]
#[ignore]
async fn test_integration() {
    // Requires live OpenSearch instance
    let config = OpenSearchConfig::new("http://localhost:9200", "test_idx", 4);
    let store = synaptic_opensearch::OpenSearchVectorStore::new(config);
    store.initialize().await.unwrap();
}
