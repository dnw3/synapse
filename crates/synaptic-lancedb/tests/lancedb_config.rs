use synaptic_lancedb::LanceDbConfig;

#[test]
fn test_table_name() {
    let config = LanceDbConfig::new("data/mydb", "documents", 768);
    assert_eq!(config.table_name, "documents");
}

#[test]
fn test_s3_uri() {
    let config = LanceDbConfig::new("s3://bucket/path", "embeddings", 1536);
    assert_eq!(config.uri, "s3://bucket/path");
}
