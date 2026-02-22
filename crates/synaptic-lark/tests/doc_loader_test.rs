use synaptic_lark::{LarkConfig, LarkDocLoader};

#[test]
fn config_defaults() {
    let config = LarkConfig::new("cli_test", "secret_test");
    assert_eq!(config.app_id, "cli_test");
    assert_eq!(config.app_secret, "secret_test");
    assert_eq!(config.base_url, "https://open.feishu.cn/open-apis");
}

#[test]
fn config_custom_base_url() {
    let config = LarkConfig::new("cli_test", "secret_test")
        .with_base_url("https://fsopen.bytedance.net/open-apis");
    assert_eq!(config.base_url, "https://fsopen.bytedance.net/open-apis");
}

#[test]
fn loader_builder() {
    let config = LarkConfig::new("cli_test", "secret_test");
    let loader = LarkDocLoader::new(config)
        .with_doc_tokens(vec!["doxcnAbc".to_string(), "doxcnDef".to_string()])
        .with_wiki_space_id("space_001");
    // Just verify it compiles and builder works without panicking
    let _ = loader;
}

#[tokio::test]
#[ignore = "requires LARK_APP_ID and LARK_APP_SECRET"]
async fn integration_load_docs() {
    let app_id = std::env::var("LARK_APP_ID").unwrap();
    let app_secret = std::env::var("LARK_APP_SECRET").unwrap();
    let doc_token = std::env::var("LARK_DOC_TOKEN").unwrap();

    let config = LarkConfig::new(app_id, app_secret);
    let loader = LarkDocLoader::new(config).with_doc_tokens(vec![doc_token]);

    use synaptic_core::Loader;
    let docs = loader.load().await.expect("load should succeed");
    assert!(!docs.is_empty());
    let doc = &docs[0];
    assert!(doc.metadata.contains_key("title"));
    assert!(doc.metadata.contains_key("source"));
    assert!(doc.content.len() > 0);
}
