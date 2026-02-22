use synaptic_langfuse::LangfuseConfig;

#[test]
fn default_host() {
    let c = LangfuseConfig::new("pk", "sk");
    assert_eq!(c.host, "https://cloud.langfuse.com");
}

#[test]
fn custom_host() {
    let c = LangfuseConfig::new("pk", "sk").with_host("https://self-hosted.example.com");
    assert_eq!(c.host, "https://self-hosted.example.com");
}

#[test]
fn flush_batch_size() {
    let c = LangfuseConfig::new("pk", "sk").with_flush_batch_size(5);
    assert_eq!(c.flush_batch_size, 5);
}

#[test]
fn keys_stored() {
    let c = LangfuseConfig::new("my-pk", "my-sk");
    assert_eq!(c.public_key, "my-pk");
    assert_eq!(c.secret_key, "my-sk");
}
