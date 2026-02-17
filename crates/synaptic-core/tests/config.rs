use synaptic_core::RunnableConfig;

#[test]
fn default_config_has_empty_fields() {
    let config = RunnableConfig::default();
    assert!(config.tags.is_empty());
    assert!(config.metadata.is_empty());
    assert!(config.max_concurrency.is_none());
    assert!(config.recursion_limit.is_none());
    assert!(config.run_id.is_none());
    assert!(config.run_name.is_none());
}

#[test]
fn config_builder_pattern() {
    let config = RunnableConfig::default()
        .with_tags(vec!["test".into()])
        .with_run_name("my-run");
    assert_eq!(config.tags, vec!["test"]);
    assert_eq!(config.run_name.as_deref(), Some("my-run"));
}
