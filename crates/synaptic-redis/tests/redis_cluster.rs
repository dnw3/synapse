//! Integration tests for Redis Cluster support.
//!
//! These tests require a running Redis Cluster and the `cluster` feature.
//! Run with: cargo test -p synaptic-redis --features cluster -- --ignored redis_cluster

#![cfg(feature = "cluster")]

use serde_json::json;
use synaptic_core::{ChatResponse, Message};
use synaptic_redis::{LlmCache, RedisCache, RedisCacheConfig, RedisStore, RedisStoreConfig, Store};

const CLUSTER_NODES: &[&str] = &[
    "redis://127.0.0.1:7000/",
    "redis://127.0.0.1:7001/",
    "redis://127.0.0.1:7002/",
];

fn cluster_store() -> RedisStore {
    let config = RedisStoreConfig {
        prefix: "synaptic:cluster:test:store:".to_string(),
    };
    RedisStore::from_cluster_nodes_with_config(CLUSTER_NODES, config)
        .expect("Cluster client creation failed")
}

fn cluster_cache() -> RedisCache {
    let config = RedisCacheConfig {
        prefix: "synaptic:cluster:test:cache:".to_string(),
        ttl: None,
    };
    RedisCache::from_cluster_nodes_with_config(CLUSTER_NODES, config)
        .expect("Cluster client creation failed")
}

// ---------------------------------------------------------------------------
// Store tests
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "requires running Redis Cluster"]
async fn redis_cluster_store_put_and_get() {
    let store = cluster_store();
    store
        .put(&["ns", "cluster"], "key1", json!("hello cluster"))
        .await
        .unwrap();

    let item = store
        .get(&["ns", "cluster"], "key1")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(item.key, "key1");
    assert_eq!(item.value, json!("hello cluster"));

    store.delete(&["ns", "cluster"], "key1").await.unwrap();
}

#[tokio::test]
#[ignore = "requires running Redis Cluster"]
async fn redis_cluster_store_search() {
    let store = cluster_store();
    store
        .put(&["ns", "csearch"], "a", json!("alpha"))
        .await
        .unwrap();
    store
        .put(&["ns", "csearch"], "b", json!("beta"))
        .await
        .unwrap();

    let all = store.search(&["ns", "csearch"], None, 10).await.unwrap();
    assert_eq!(all.len(), 2);

    let filtered = store
        .search(&["ns", "csearch"], Some("alpha"), 10)
        .await
        .unwrap();
    assert_eq!(filtered.len(), 1);

    for key in ["a", "b"] {
        store.delete(&["ns", "csearch"], key).await.unwrap();
    }
}

// ---------------------------------------------------------------------------
// Cache tests
// ---------------------------------------------------------------------------

#[tokio::test]
#[ignore = "requires running Redis Cluster"]
async fn redis_cluster_cache_put_and_get() {
    let cache = cluster_cache();
    let response = ChatResponse {
        message: Message::ai("cluster cached"),
        usage: None,
    };

    cache.put("cluster_key", &response).await.unwrap();
    let cached = cache.get("cluster_key").await.unwrap().unwrap();
    assert_eq!(cached.message.content(), "cluster cached");

    cache.clear().await.unwrap();
}

#[tokio::test]
#[ignore = "requires running Redis Cluster"]
async fn redis_cluster_cache_clear() {
    let cache = cluster_cache();
    let response = ChatResponse {
        message: Message::ai("to be cleared"),
        usage: None,
    };

    cache.put("cc_key_1", &response).await.unwrap();
    cache.put("cc_key_2", &response).await.unwrap();

    cache.clear().await.unwrap();

    assert!(cache.get("cc_key_1").await.unwrap().is_none());
    assert!(cache.get("cc_key_2").await.unwrap().is_none());
}
