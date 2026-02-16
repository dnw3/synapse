use synapse_loaders::{Loader, TextLoader};

#[tokio::test]
async fn text_loader_returns_single_document() {
    let loader = TextLoader::new("doc-1", "hello world");
    let docs = loader.load().await.expect("load should work");

    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].id, "doc-1");
    assert_eq!(docs[0].content, "hello world");
}
