use synaptic_lancedb::LanceDbConfig;

#[test]
fn test_config_new() {
    let config = LanceDbConfig::new("/tmp/test_db", "test_table", 1536);
    assert_eq!(config.uri, "/tmp/test_db");
    assert_eq!(config.table_name, "test_table");
    assert_eq!(config.dim, 1536);
}

#[tokio::test]
async fn test_create_store() {
    let config = LanceDbConfig::new("/tmp/synaptic_test_lancedb", "test", 4);
    let store = synaptic_lancedb::LanceDbVectorStore::new(config)
        .await
        .unwrap();
    assert_eq!(store.config().table_name, "test");
}
