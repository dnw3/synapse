use synaptic_opensearch::OpenSearchConfig;

#[test]
fn test_no_credentials_by_default() {
    let config = OpenSearchConfig::new("http://host:9200", "idx", 512);
    assert!(config.username.is_none());
    assert!(config.password.is_none());
}

#[test]
fn test_dim_stored() {
    let config = OpenSearchConfig::new("http://host:9200", "idx", 1536);
    assert_eq!(config.dim, 1536);
}
