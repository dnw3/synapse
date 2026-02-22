use synaptic_loaders::NotionLoader;

#[test]
fn test_notion_loader_new() {
    let _loader = NotionLoader::new("test-token", vec!["page-id-1".to_string()]);
}

#[tokio::test]
#[ignore]
async fn test_load_pages_integration() {
    let token = std::env::var("NOTION_TOKEN").unwrap();
    let page_id = std::env::var("NOTION_PAGE_ID").unwrap();
    let loader = NotionLoader::new(token, vec![page_id]);
    use synaptic_core::Loader;
    let docs = loader.load().await.unwrap();
    assert!(!docs.is_empty());
}
