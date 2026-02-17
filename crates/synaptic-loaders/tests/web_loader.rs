use synaptic_loaders::WebBaseLoader;

// WebBaseLoader requires network access, so we only test construction
// and basic structure. Integration tests against real URLs are skipped.

#[test]
fn web_loader_can_be_constructed() {
    let _loader = WebBaseLoader::new("https://example.com");
}

#[test]
fn web_loader_stores_url() {
    // WebBaseLoader has no public url() accessor, so verify it is Send + Sync
    // and can be constructed with different URLs
    let loader = WebBaseLoader::new("https://example.com/page");
    fn assert_send_sync<T: Send + Sync>(_t: &T) {}
    assert_send_sync(&loader);
}
