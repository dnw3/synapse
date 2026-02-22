use synaptic_slack::SlackConfig;

#[test]
fn test_multiple_channels() {
    let config = SlackConfig::new(
        "token",
        vec!["C1".to_string(), "C2".to_string(), "C3".to_string()],
    );
    assert_eq!(config.channel_ids.len(), 3);
}

#[test]
fn test_default_no_threads() {
    let config = SlackConfig::new("token", vec![]);
    assert!(!config.include_threads);
}
