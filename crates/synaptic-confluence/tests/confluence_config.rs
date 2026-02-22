use synaptic_confluence::ConfluenceConfig;

#[test]
fn test_api_token_stored() {
    let config = ConfluenceConfig::new("domain.atlassian.net", "email@co.com", "secret-token");
    assert_eq!(config.api_token, "secret-token");
}

#[test]
fn test_multiple_page_ids() {
    let ids = vec!["1".to_string(), "2".to_string(), "3".to_string()];
    let config =
        ConfluenceConfig::new("d.atlassian.net", "e@c.com", "t").with_page_ids(ids.clone());
    assert_eq!(config.page_ids, ids);
}
