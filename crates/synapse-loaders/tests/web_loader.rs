use synapse_loaders::WebBaseLoader;

// WebBaseLoader requires network access, so we only test construction
// and basic structure. Integration tests against real URLs are skipped.

#[test]
fn web_loader_can_be_constructed() {
    let _loader = WebBaseLoader::new("https://example.com");
}
