use futures::StreamExt;
use synapse_loaders::{Loader, TextLoader};

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
