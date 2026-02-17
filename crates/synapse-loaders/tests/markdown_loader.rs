use std::fs;
use std::sync::atomic::{AtomicUsize, Ordering};

use synapse_loaders::{Loader, MarkdownLoader};

static COUNTER: AtomicUsize = AtomicUsize::new(0);

fn tempfile(content: &str) -> std::path::PathBuf {
    let n = COUNTER.fetch_add(1, Ordering::SeqCst);
    let path = std::env::temp_dir().join(format!(
        "synapse-md-loader-test-{}-{}.md",
        std::process::id(),
        n
    ));
    fs::write(&path, content).unwrap();
    path
}

#[tokio::test]
async fn loads_markdown_content() {
    let md = "# Title\n\nSome **bold** text.\n\n## Section\n\nMore content.";
    let path = tempfile(md);
    let loader = MarkdownLoader::new(&path);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].content, md);
}

#[tokio::test]
async fn adds_format_and_source_metadata() {
    let path = tempfile("# Hello");
    let loader = MarkdownLoader::new(&path);
    let docs = loader.load().await.unwrap();

    assert_eq!(
        docs[0].metadata.get("format").unwrap(),
        &serde_json::Value::String("markdown".to_string())
    );
    assert_eq!(
        docs[0].metadata.get("source").unwrap(),
        &serde_json::Value::String(path.to_string_lossy().to_string())
    );
}

#[tokio::test]
async fn returns_error_for_missing_file() {
    let loader = MarkdownLoader::new("/tmp/nonexistent-synapse-md-test-12345.md");
    let result = loader.load().await;
    assert!(result.is_err());
}
