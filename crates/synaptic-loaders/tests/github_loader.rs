use synaptic_loaders::GitHubLoader;

#[test]
fn test_github_loader_new() {
    let _loader = GitHubLoader::new("owner", "repo", vec!["README.md".to_string()]);
}

#[test]
fn test_github_loader_with_token() {
    let loader = GitHubLoader::new("owner", "repo", vec![])
        .with_token("ghp-test")
        .with_branch("main")
        .with_extensions(vec![".rs".to_string()]);
    let _ = loader;
}

#[tokio::test]
#[ignore]
async fn test_load_file_integration() {
    let loader = GitHubLoader::new("rust-lang", "rust", vec!["README.md".to_string()]);
    use synaptic_core::Loader;
    let docs = loader.load().await.unwrap();
    assert!(!docs.is_empty());
    assert!(!docs[0].content.is_empty());
}
