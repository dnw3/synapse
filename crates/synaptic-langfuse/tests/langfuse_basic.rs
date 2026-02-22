use synaptic_langfuse::{LangfuseCallback, LangfuseConfig};

#[tokio::test]
async fn callback_creation() {
    let config = LangfuseConfig::new("pk-lf-test", "sk-lf-test");
    let _cb = LangfuseCallback::new(config).await.unwrap();
}

#[tokio::test]
async fn config_builder() {
    let config = LangfuseConfig::new("pk", "sk")
        .with_host("https://langfuse.example.com")
        .with_flush_batch_size(10);
    assert_eq!(config.host, "https://langfuse.example.com");
    assert_eq!(config.flush_batch_size, 10);
}

#[tokio::test]
async fn flush_empty_noop() {
    let config = LangfuseConfig::new("pk-lf-test", "sk-lf-test");
    let cb = LangfuseCallback::new(config).await.unwrap();
    cb.flush().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn flush_integration() {
    let public_key = std::env::var("LANGFUSE_PUBLIC_KEY").unwrap();
    let secret_key = std::env::var("LANGFUSE_SECRET_KEY").unwrap();
    let config = LangfuseConfig::new(public_key, secret_key);
    let cb = LangfuseCallback::new(config).await.unwrap();
    cb.flush().await.unwrap();
}
