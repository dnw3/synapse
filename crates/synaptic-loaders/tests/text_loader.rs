use synaptic_loaders::{Loader, TextLoader};

#[tokio::test]
async fn text_loader_returns_single_document() {
    let loader = TextLoader::new("doc-1", "hello world");
    let docs = loader.load().await.expect("load should work");

    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].id, "doc-1");
    assert_eq!(docs[0].content, "hello world");
}

#[tokio::test]
async fn text_loader_preserves_id() {
    let loader = TextLoader::new("custom-id-123", "content");
    let docs = loader.load().await.expect("load");
    assert_eq!(docs[0].id, "custom-id-123");
}

#[tokio::test]
async fn text_loader_empty_content() {
    let loader = TextLoader::new("empty", "");
    let docs = loader.load().await.expect("load");
    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].content, "");
}

#[tokio::test]
async fn text_loader_unicode_content() {
    let loader = TextLoader::new("unicode", "こんにちは世界 🌍 Ñoño");
    let docs = loader.load().await.expect("load");
    assert_eq!(docs[0].content, "こんにちは世界 🌍 Ñoño");
}
