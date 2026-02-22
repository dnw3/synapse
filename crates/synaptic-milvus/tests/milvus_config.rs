use synaptic_milvus::MilvusConfig;

#[test]
fn test_dim_stored() {
    let config = MilvusConfig::new("http://host:19530", "coll", 1024);
    assert_eq!(config.dim, 1024);
}

#[test]
fn test_collection_name() {
    let config = MilvusConfig::new("http://host:19530", "my_collection", 512);
    assert_eq!(config.collection, "my_collection");
}
