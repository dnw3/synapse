use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};

use synapse_loaders::{FileLoader, Loader};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn tempfile(content: &str) -> std::path::PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let path = std::env::temp_dir().join(format!(
        "synapse-file-loader-test-{}-{}.txt",
        std::process::id(),
        n
    ));
    fs::write(&path, content).unwrap();
    path
}

#[tokio::test]
async fn loads_file_content() {
    let path = tempfile("Hello from file loader!");
    let loader = FileLoader::new(&path);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].content, "Hello from file loader!");
    assert_eq!(docs[0].id, path.to_string_lossy().to_string());
}

#[tokio::test]
async fn adds_source_metadata() {
    let path = tempfile("metadata test");
    let loader = FileLoader::new(&path);
    let docs = loader.load().await.unwrap();

    assert_eq!(
        docs[0].metadata.get("source").unwrap(),
        &serde_json::Value::String(path.to_string_lossy().to_string())
    );
}

#[tokio::test]
async fn returns_error_for_missing_file() {
    let loader = FileLoader::new("/tmp/nonexistent-synapse-test-file-12345.txt");
    let result = loader.load().await;
    assert!(result.is_err());
}
