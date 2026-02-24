//! Integration tests for `PgCache`.
//!
//! The `#[ignore]` tests require a running PostgreSQL instance. Set the
//! `DATABASE_URL` environment variable to the connection string before running:
//!
//! ```bash
//! DATABASE_URL=postgres://user:pass@localhost/test_db cargo test -p synaptic-postgres -- --ignored cache
//! ```

use synaptic_core::Message;
use synaptic_postgres::{ChatResponse, LlmCache, PgCache, PgCacheConfig};

fn make_response(content: &str) -> ChatResponse {
    ChatResponse {
        message: Message::ai(content),
        usage: None,
    }
}

async fn setup_cache(table_name: &str) -> PgCache {
    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for postgres tests");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await
        .expect("failed to connect to PostgreSQL");

    let drop_sql = format!("DROP TABLE IF EXISTS {table_name}");
    sqlx::query(&drop_sql)
        .execute(&pool)
        .await
        .expect("failed to drop test table");

    let config = PgCacheConfig::new(table_name);
    let cache = PgCache::new(pool, config);
    cache.initialize().await.expect("initialize failed");
    cache
}

#[tokio::test]
#[ignore]
async fn test_put_and_get() {
    let cache = setup_cache("test_cache_put_get").await;

    cache.put("key1", &make_response("hello")).await.unwrap();
    let result = cache.get("key1").await.unwrap();
    assert!(result.is_some());
    assert_eq!(result.unwrap().message.content(), "hello");
}

#[tokio::test]
#[ignore]
async fn test_get_missing() {
    let cache = setup_cache("test_cache_get_missing").await;
    let result = cache.get("nonexistent").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
#[ignore]
async fn test_clear() {
    let cache = setup_cache("test_cache_clear").await;

    cache.put("k1", &make_response("a")).await.unwrap();
    cache.put("k2", &make_response("b")).await.unwrap();

    cache.clear().await.unwrap();

    assert!(cache.get("k1").await.unwrap().is_none());
    assert!(cache.get("k2").await.unwrap().is_none());
}

#[tokio::test]
#[ignore]
async fn test_put_overwrites() {
    let cache = setup_cache("test_cache_put_overwrites").await;

    cache.put("k1", &make_response("first")).await.unwrap();
    cache.put("k1", &make_response("second")).await.unwrap();

    let result = cache.get("k1").await.unwrap().unwrap();
    assert_eq!(result.message.content(), "second");
}
