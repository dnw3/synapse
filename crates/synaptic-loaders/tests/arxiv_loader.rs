use synaptic_loaders::ArxivLoader;

#[test]
fn test_arxiv_loader_new() {
    let loader = ArxivLoader::new("rust programming language").with_max_results(5);
    let _ = loader;
}

#[tokio::test]
#[ignore]
async fn test_load_papers_integration() {
    let loader = ArxivLoader::new("large language models").with_max_results(3);
    use synaptic_core::Loader;
    let docs = loader.load().await.unwrap();
    assert_eq!(docs.len(), 3);
    assert!(!docs[0].content.is_empty());
}
