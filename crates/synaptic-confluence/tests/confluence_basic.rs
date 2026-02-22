use synaptic_confluence::ConfluenceConfig;

#[test]
fn test_config_new() {
    let config = ConfluenceConfig::new("company.atlassian.net", "user@example.com", "api-token");
    assert_eq!(config.domain, "company.atlassian.net");
    assert!(config.space_key.is_none());
    assert!(config.page_ids.is_empty());
}

#[test]
fn test_config_builder() {
    let config = ConfluenceConfig::new("co.atlassian.net", "user@co.com", "tok")
        .with_space_key("PROJ")
        .with_page_ids(vec!["123".to_string(), "456".to_string()]);
    assert_eq!(config.space_key, Some("PROJ".to_string()));
    assert_eq!(config.page_ids.len(), 2);
}

#[tokio::test]
#[ignore]
async fn test_load_pages_integration() {
    // Requires live Confluence instance
    let config = ConfluenceConfig::new(
        std::env::var("CONFLUENCE_DOMAIN").unwrap(),
        std::env::var("CONFLUENCE_EMAIL").unwrap(),
        std::env::var("CONFLUENCE_TOKEN").unwrap(),
    )
    .with_page_ids(vec!["some-page-id".to_string()]);
    let loader = synaptic_confluence::ConfluenceLoader::new(config);
    use synaptic_core::Loader;
    let docs = loader.load().await.unwrap();
    assert!(!docs.is_empty());
}
