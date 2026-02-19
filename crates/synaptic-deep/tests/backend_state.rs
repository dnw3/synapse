use synaptic_deep::backend::{Backend, GrepOutputMode, StateBackend};

#[tokio::test]
async fn write_and_read_file() {
    let backend = StateBackend::new();
    backend.write_file("hello.txt", "line1\nline2\nline3").await.unwrap();
    let content = backend.read_file("hello.txt", 0, 2000).await.unwrap();
    assert_eq!(content, "line1\nline2\nline3");
}

#[tokio::test]
async fn read_file_not_found() {
    let backend = StateBackend::new();
    let err = backend.read_file("missing.txt", 0, 100).await;
    assert!(err.is_err());
}

#[tokio::test]
async fn read_file_with_offset_and_limit() {
    let backend = StateBackend::new();
    backend.write_file("nums.txt", "a\nb\nc\nd\ne").await.unwrap();

    let content = backend.read_file("nums.txt", 1, 2).await.unwrap();
    assert_eq!(content, "b\nc");

    let content = backend.read_file("nums.txt", 4, 100).await.unwrap();
    assert_eq!(content, "e");

    let content = backend.read_file("nums.txt", 100, 100).await.unwrap();
    assert_eq!(content, "");
}

#[tokio::test]
async fn edit_file_single_replacement() {
    let backend = StateBackend::new();
    backend.write_file("f.txt", "hello world hello").await.unwrap();
    backend.edit_file("f.txt", "hello", "hi", false).await.unwrap();
    let content = backend.read_file("f.txt", 0, 100).await.unwrap();
    assert_eq!(content, "hi world hello");
}

#[tokio::test]
async fn edit_file_replace_all() {
    let backend = StateBackend::new();
    backend.write_file("f.txt", "aaa bbb aaa").await.unwrap();
    backend.edit_file("f.txt", "aaa", "ccc", true).await.unwrap();
    let content = backend.read_file("f.txt", 0, 100).await.unwrap();
    assert_eq!(content, "ccc bbb ccc");
}

#[tokio::test]
async fn edit_file_not_found() {
    let backend = StateBackend::new();
    backend.write_file("f.txt", "content").await.unwrap();
    let err = backend.edit_file("f.txt", "missing", "x", false).await;
    assert!(err.is_err());
}

#[tokio::test]
async fn ls_root() {
    let backend = StateBackend::new();
    backend.write_file("a.txt", "").await.unwrap();
    backend.write_file("dir/b.txt", "").await.unwrap();
    backend.write_file("dir/c.txt", "").await.unwrap();

    let entries = backend.ls(".").await.unwrap();
    assert_eq!(entries.len(), 2);
    assert_eq!(entries[0].name, "a.txt");
    assert!(!entries[0].is_dir);
    assert_eq!(entries[1].name, "dir");
    assert!(entries[1].is_dir);
}

#[tokio::test]
async fn ls_subdirectory() {
    let backend = StateBackend::new();
    backend.write_file("src/main.rs", "fn main() {}").await.unwrap();
    backend.write_file("src/lib.rs", "").await.unwrap();
    backend.write_file("src/utils/helper.rs", "").await.unwrap();

    let entries = backend.ls("src").await.unwrap();
    assert_eq!(entries.len(), 3);
    assert_eq!(entries[0].name, "lib.rs");
    assert_eq!(entries[1].name, "main.rs");
    assert_eq!(entries[2].name, "utils");
    assert!(entries[2].is_dir);
}

#[tokio::test]
async fn glob_star() {
    let backend = StateBackend::new();
    backend.write_file("src/main.rs", "").await.unwrap();
    backend.write_file("src/lib.rs", "").await.unwrap();
    backend.write_file("src/test.txt", "").await.unwrap();

    let matches = backend.glob("*.rs", "src").await.unwrap();
    assert_eq!(matches.len(), 2);
    assert!(matches.contains(&"src/lib.rs".to_string()));
    assert!(matches.contains(&"src/main.rs".to_string()));
}

#[tokio::test]
async fn glob_double_star() {
    let backend = StateBackend::new();
    backend.write_file("a.rs", "").await.unwrap();
    backend.write_file("src/b.rs", "").await.unwrap();
    backend.write_file("src/deep/c.rs", "").await.unwrap();

    let matches = backend.glob("**/*.rs", ".").await.unwrap();
    // Should match files with at least one directory component
    assert!(matches.contains(&"src/b.rs".to_string()));
    assert!(matches.contains(&"src/deep/c.rs".to_string()));
}

#[tokio::test]
async fn grep_files_with_matches() {
    let backend = StateBackend::new();
    backend.write_file("a.txt", "hello world").await.unwrap();
    backend.write_file("b.txt", "goodbye world").await.unwrap();
    backend.write_file("c.txt", "no match here").await.unwrap();

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
async fn grep_content_mode() {
    let backend = StateBackend::new();
    backend.write_file("f.txt", "line1\nfoo bar\nline3").await.unwrap();

    let result = backend
        .grep("foo", None, None, GrepOutputMode::Content)
        .await
        .unwrap();
    assert_eq!(result, "f.txt:2:foo bar");
}

#[tokio::test]
async fn grep_count_mode() {
    let backend = StateBackend::new();
    backend.write_file("f.txt", "aaa\nbbb\naaa").await.unwrap();

    let result = backend
        .grep("aaa", None, None, GrepOutputMode::Count)
        .await
        .unwrap();
    assert_eq!(result, "f.txt:2");
}

#[tokio::test]
async fn grep_with_path_filter() {
    let backend = StateBackend::new();
    backend.write_file("src/a.rs", "fn main()").await.unwrap();
    backend.write_file("tests/b.rs", "fn main()").await.unwrap();

    let result = backend
        .grep("main", Some("src"), None, GrepOutputMode::FilesWithMatches)
        .await
        .unwrap();
    assert_eq!(result, "src/a.rs");
}

#[tokio::test]
async fn execute_not_supported() {
    let backend = StateBackend::new();
    assert!(!backend.supports_execution());
    let err = backend.execute("ls", None).await;
    assert!(err.is_err());
}

#[tokio::test]
async fn path_normalization() {
    let backend = StateBackend::new();
    backend.write_file("/a/b.txt", "content").await.unwrap();
    let content = backend.read_file("a/b.txt", 0, 100).await.unwrap();
    assert_eq!(content, "content");
}
