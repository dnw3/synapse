use synaptic_slack::SlackConfig;

#[test]
fn test_config_new() {
    let config = SlackConfig::new("xoxb-test-token", vec!["C123456".to_string()]);
    assert_eq!(config.bot_token, "xoxb-test-token");
    assert_eq!(config.channel_ids, vec!["C123456"]);
    assert_eq!(config.limit, 100);
    assert!(!config.include_threads);
}

#[test]
fn test_config_builder() {
    let config = SlackConfig::new("token", vec!["C1".to_string()])
        .with_limit(50)
        .with_oldest("1614556800.000000")
        .with_threads();
    assert_eq!(config.limit, 50);
    assert_eq!(config.oldest, Some("1614556800.000000".to_string()));
    assert!(config.include_threads);
}

#[tokio::test]
#[ignore]
async fn test_load_messages_integration() {
    let bot_token = std::env::var("SLACK_BOT_TOKEN").unwrap();
    let channel_id = std::env::var("SLACK_CHANNEL_ID").unwrap();
    let config = SlackConfig::new(bot_token, vec![channel_id]).with_limit(10);
    let loader = synaptic_slack::SlackLoader::new(config);
    use synaptic_core::Loader;
    let docs = loader.load().await.unwrap();
    println!("Loaded {} messages", docs.len());
}
