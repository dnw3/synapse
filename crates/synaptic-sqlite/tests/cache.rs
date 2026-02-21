use synaptic_core::{ChatResponse, LlmCache, Message, TokenUsage};
use synaptic_sqlite::{SqliteCache, SqliteCacheConfig};

fn make_response(content: &str) -> ChatResponse {
    ChatResponse {
        message: Message::ai(content),
        usage: Some(TokenUsage {
            input_tokens: 10,
            output_tokens: 5,
            total_tokens: 15,
            input_details: None,
            output_details: None,
        }),
    }
}

#[tokio::test]
async fn put_and_get() {
    let cache = SqliteCache::new(SqliteCacheConfig::in_memory()).unwrap();

    cache.put("key1", &make_response("hello")).await.unwrap();
    let result = cache.get("key1").await.unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap().message.content(), "hello");
}

#[tokio::test]
async fn get_missing_key() {
    let cache = SqliteCache::new(SqliteCacheConfig::in_memory()).unwrap();
    let result = cache.get("nonexistent").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn clear_removes_all() {
    let cache = SqliteCache::new(SqliteCacheConfig::in_memory()).unwrap();
    cache.put("k1", &make_response("a")).await.unwrap();
    cache.put("k2", &make_response("b")).await.unwrap();
    cache.clear().await.unwrap();
    assert!(cache.get("k1").await.unwrap().is_none());
    assert!(cache.get("k2").await.unwrap().is_none());
}

#[tokio::test]
async fn put_overwrites() {
    let cache = SqliteCache::new(SqliteCacheConfig::in_memory()).unwrap();
    cache.put("k1", &make_response("first")).await.unwrap();
    cache.put("k1", &make_response("second")).await.unwrap();
    let result = cache.get("k1").await.unwrap().unwrap();
    assert_eq!(result.message.content(), "second");
}

#[tokio::test]
async fn preserves_usage_metadata() {
    let cache = SqliteCache::new(SqliteCacheConfig::in_memory()).unwrap();
    let response = make_response("with usage");

    cache.put("k1", &response).await.unwrap();
    let cached = cache.get("k1").await.unwrap().unwrap();

    let usage = cached.usage.unwrap();
    assert_eq!(usage.input_tokens, 10);
    assert_eq!(usage.output_tokens, 5);
    assert_eq!(usage.total_tokens, 15);
}

#[test]
fn config_builder() {
    let config = SqliteCacheConfig::new("/tmp/test.db").with_ttl(3600);
    assert_eq!(config.path, "/tmp/test.db");
    assert_eq!(config.ttl, Some(3600));
}

#[test]
fn config_in_memory() {
    let config = SqliteCacheConfig::in_memory();
    assert_eq!(config.path, ":memory:");
    assert!(config.ttl.is_none());
}

#[test]
fn config_default_no_ttl() {
    let config = SqliteCacheConfig::new("test.db");
    assert!(config.ttl.is_none());
}
