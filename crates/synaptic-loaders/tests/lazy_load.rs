use futures::StreamExt;
use synaptic_loaders::{Loader, TextLoader};

#[tokio::test]
async fn lazy_load_yields_all_documents() {
    let loader = TextLoader::new("doc-1", "hello world");
    let mut stream = loader.lazy_load();

    let mut docs = Vec::new();
    while let Some(result) = stream.next().await {
        docs.push(result.unwrap());
    }

    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].id, "doc-1");
    assert_eq!(docs[0].content, "hello world");
}

#[tokio::test]
async fn lazy_load_empty_loader() {
    // TextLoader always returns 1 doc, but we can test stream mechanics
    let loader = TextLoader::new("single", "content");
    let mut stream = loader.lazy_load();
    let first = stream.next().await;
    assert!(first.is_some());
    let second = stream.next().await;
    assert!(second.is_none()); // Stream exhausted
}

#[tokio::test]
async fn lazy_load_content_matches_load() {
    let loader = TextLoader::new("doc-1", "hello world");

    // Collect from stream
    let mut stream = loader.lazy_load();
    let mut stream_docs = Vec::new();
    while let Some(result) = stream.next().await {
        stream_docs.push(result.unwrap());
    }

    // Collect from load
    let load_docs = loader.load().await.unwrap();

    assert_eq!(stream_docs.len(), load_docs.len());
    assert_eq!(stream_docs[0].id, load_docs[0].id);
    assert_eq!(stream_docs[0].content, load_docs[0].content);
}
