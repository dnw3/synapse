use std::sync::Arc;
use synaptic_deep::backend::{Backend, GrepOutputMode, StoreBackend};
use synaptic_store::InMemoryStore;

#[tokio::test]
async fn write_and_read_file() {
    let store = Arc::new(InMemoryStore::new());
    let backend = StoreBackend::new(store, vec!["workspace".to_string()]);

    backend
        .write_file("hello.txt", "Hello World")
        .await
        .unwrap();
    let content = backend.read_file("hello.txt", 0, 100).await.unwrap();
    assert_eq!(content, "Hello World");
}

#[tokio::test]
async fn edit_file_replaces_text() {
    let store = Arc::new(InMemoryStore::new());
    let backend = StoreBackend::new(store, vec!["ws".to_string()]);

    backend
        .write_file("test.txt", "Hello World")
        .await
        .unwrap();
    backend
        .edit_file("test.txt", "World", "Rust", false)
        .await
        .unwrap();
    let content = backend.read_file("test.txt", 0, 100).await.unwrap();
    assert_eq!(content, "Hello Rust");
}

#[tokio::test]
async fn edit_file_replace_all() {
    let store = Arc::new(InMemoryStore::new());
    let backend = StoreBackend::new(store, vec!["ws".to_string()]);

    backend
        .write_file("dup.txt", "aaa bbb aaa")
        .await
        .unwrap();
    backend
        .edit_file("dup.txt", "aaa", "ccc", true)
        .await
        .unwrap();
    let content = backend.read_file("dup.txt", 0, 100).await.unwrap();
    assert_eq!(content, "ccc bbb ccc");
}

#[tokio::test]
async fn ls_lists_files_in_root() {
    let store = Arc::new(InMemoryStore::new());
    let backend = StoreBackend::new(store, vec!["ws".to_string()]);

    backend.write_file("a.txt", "a").await.unwrap();
    backend.write_file("b.txt", "b").await.unwrap();

    let entries = backend.ls(".").await.unwrap();
    let names: Vec<&str> = entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"a.txt"));
    assert!(names.contains(&"b.txt"));
    assert!(entries.len() >= 2);
}

#[tokio::test]
async fn ls_shows_directories() {
    let store = Arc::new(InMemoryStore::new());
    let backend = StoreBackend::new(store, vec!["ws".to_string()]);

    backend
        .write_file("src/main.rs", "fn main() {}")
        .await
        .unwrap();
    backend.write_file("README.md", "# hi").await.unwrap();

    let entries = backend.ls(".").await.unwrap();
    let dir_entry = entries.iter().find(|e| e.name == "src");
    assert!(dir_entry.is_some());
    assert!(dir_entry.unwrap().is_dir);

    let file_entry = entries.iter().find(|e| e.name == "README.md");
    assert!(file_entry.is_some());
    assert!(!file_entry.unwrap().is_dir);
}

#[tokio::test]
async fn read_nonexistent_file_errors() {
    let store = Arc::new(InMemoryStore::new());
    let backend = StoreBackend::new(store, vec!["ws".to_string()]);

    let result = backend.read_file("missing.txt", 0, 100).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn edit_nonexistent_file_errors() {
    let store = Arc::new(InMemoryStore::new());
    let backend = StoreBackend::new(store, vec!["ws".to_string()]);

    let result = backend.edit_file("missing.txt", "old", "new", false).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn edit_file_old_text_not_found_errors() {
    let store = Arc::new(InMemoryStore::new());
    let backend = StoreBackend::new(store, vec!["ws".to_string()]);

    backend
        .write_file("f.txt", "some content")
        .await
        .unwrap();
    let result = backend
        .edit_file("f.txt", "nonexistent text", "replacement", false)
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn glob_pattern_matching() {
    let store = Arc::new(InMemoryStore::new());
    let backend = StoreBackend::new(store, vec!["ws".to_string()]);

    backend
        .write_file("src/main.rs", "fn main() {}")
        .await
        .unwrap();
    backend
        .write_file("src/lib.rs", "pub mod lib;")
        .await
        .unwrap();
    backend
        .write_file("README.md", "# Hello")
        .await
        .unwrap();

    let matches = backend.glob("*.rs", "src").await.unwrap();
    assert_eq!(matches.len(), 2);
    assert!(matches.contains(&"src/lib.rs".to_string()));
    assert!(matches.contains(&"src/main.rs".to_string()));
}

#[tokio::test]
async fn no_execution_support() {
    let store = Arc::new(InMemoryStore::new());
    let backend = StoreBackend::new(store, vec!["ws".to_string()]);

    assert!(!backend.supports_execution());
    let result = backend.execute("echo hello", None).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn grep_content_mode() {
    let store = Arc::new(InMemoryStore::new());
    let backend = StoreBackend::new(store, vec!["ws".to_string()]);

    backend
        .write_file("test.txt", "hello world\nfoo bar\nhello again")
        .await
        .unwrap();

    let result = backend
        .grep("hello", Some("test.txt"), None, GrepOutputMode::Content)
        .await
        .unwrap();
    assert!(result.contains("hello"));
    // Should match two lines
    let lines: Vec<&str> = result.lines().collect();
    assert_eq!(lines.len(), 2);
}

#[tokio::test]
async fn grep_files_with_matches_mode() {
    let store = Arc::new(InMemoryStore::new());
    let backend = StoreBackend::new(store, vec!["ws".to_string()]);

    backend
        .write_file("a.txt", "hello world")
        .await
        .unwrap();
    backend
        .write_file("b.txt", "goodbye world")
        .await
        .unwrap();
    backend
        .write_file("c.txt", "no match here")
        .await
        .unwrap();

    let result = backend
        .grep("world", None, None, GrepOutputMode::FilesWithMatches)
        .await
        .unwrap();
    let files: Vec<&str> = result.lines().collect();
    assert_eq!(files.len(), 2);
    assert!(files.contains(&"a.txt"));
    assert!(files.contains(&"b.txt"));
}

#[tokio::test]
async fn grep_count_mode() {
    let store = Arc::new(InMemoryStore::new());
    let backend = StoreBackend::new(store, vec!["ws".to_string()]);

    backend
        .write_file("f.txt", "aaa\nbbb\naaa")
        .await
        .unwrap();

    let result = backend
        .grep("aaa", None, None, GrepOutputMode::Count)
        .await
        .unwrap();
    assert_eq!(result, "f.txt:2");
}

#[tokio::test]
async fn read_file_with_offset_and_limit() {
    let store = Arc::new(InMemoryStore::new());
    let backend = StoreBackend::new(store, vec!["ws".to_string()]);

    backend
        .write_file("lines.txt", "line0\nline1\nline2\nline3\nline4")
        .await
        .unwrap();

    // Read lines 1..3
    let content = backend.read_file("lines.txt", 1, 2).await.unwrap();
    assert_eq!(content, "line1\nline2");

    // Offset past end returns empty
    let content = backend.read_file("lines.txt", 100, 10).await.unwrap();
    assert_eq!(content, "");
}

#[tokio::test]
async fn write_overwrites_existing_file() {
    let store = Arc::new(InMemoryStore::new());
    let backend = StoreBackend::new(store, vec!["ws".to_string()]);

    backend.write_file("f.txt", "original").await.unwrap();
    backend.write_file("f.txt", "updated").await.unwrap();

    let content = backend.read_file("f.txt", 0, 100).await.unwrap();
    assert_eq!(content, "updated");
}
