use synaptic_redis::{RedisCacheConfig, RedisStoreConfig};

// ---------------------------------------------------------------------------
// Unit tests for config construction (no Redis required)
// ---------------------------------------------------------------------------

#[test]
fn store_config_defaults() {
    let config = RedisStoreConfig::default();
    assert_eq!(config.prefix, "synaptic:store:");
}

#[test]
fn cache_config_defaults() {
    let config = RedisCacheConfig::default();
    assert_eq!(config.prefix, "synaptic:cache:");
    assert!(config.ttl.is_none());
}

#[test]
fn cache_config_with_ttl() {
    let config = RedisCacheConfig {
        ttl: Some(3600),
        ..Default::default()
    };
    assert_eq!(config.ttl, Some(3600));
    assert_eq!(config.prefix, "synaptic:cache:");
}

#[test]
fn store_config_custom_prefix() {
    let config = RedisStoreConfig {
        prefix: "myapp:".to_string(),
    };
    assert_eq!(config.prefix, "myapp:");
}

#[test]
fn store_from_url_valid() {
    let result = synaptic_redis::RedisStore::from_url("redis://127.0.0.1/");
    assert!(result.is_ok());
}

#[test]
fn store_from_url_invalid_url() {
    // An obviously invalid URL should produce an error
    let result = synaptic_redis::RedisStore::from_url("not-a-valid-url");
    assert!(result.is_err());
}

#[test]
fn cache_from_url_valid() {
    let result = synaptic_redis::RedisCache::from_url("redis://127.0.0.1/");
    assert!(result.is_ok());
}

#[test]
fn cache_from_url_invalid_url() {
    let result = synaptic_redis::RedisCache::from_url("not-a-valid-url");
    assert!(result.is_err());
}

// ---------------------------------------------------------------------------
// Cluster constructor tests (no running cluster required)
// ---------------------------------------------------------------------------

#[cfg(feature = "cluster")]
mod cluster_constructors {
    use synaptic_redis::{RedisCache, RedisCacheConfig, RedisStore, RedisStoreConfig};

    #[test]
    fn store_from_cluster_nodes() {
        let result = RedisStore::from_cluster_nodes(&[
            "redis://127.0.0.1:7000/",
            "redis://127.0.0.1:7001/",
            "redis://127.0.0.1:7002/",
        ]);
        assert!(result.is_ok());
    }

    #[test]
    fn store_from_cluster_nodes_with_config() {
        let config = RedisStoreConfig {
            prefix: "cluster:store:".to_string(),
        };
        let result = RedisStore::from_cluster_nodes_with_config(
            &["redis://127.0.0.1:7000/", "redis://127.0.0.1:7001/"],
            config,
        );
        assert!(result.is_ok());
    }

    #[test]
    fn cache_from_cluster_nodes() {
        let result =
            RedisCache::from_cluster_nodes(&["redis://127.0.0.1:7000/", "redis://127.0.0.1:7001/"]);
        assert!(result.is_ok());
    }

    #[test]
    fn cache_from_cluster_nodes_with_config() {
        let config = RedisCacheConfig {
            prefix: "cluster:cache:".to_string(),
            ttl: Some(600),
        };
        let result = RedisCache::from_cluster_nodes_with_config(
            &["redis://127.0.0.1:7000/", "redis://127.0.0.1:7001/"],
            config,
        );
        assert!(result.is_ok());
    }
}

// ---------------------------------------------------------------------------
// Integration tests — require a running Redis instance.
// Run with: cargo test -p synaptic-redis -- --ignored
// ---------------------------------------------------------------------------

#[cfg(test)]
mod integration {
    use serde_json::json;
    use synaptic_core::{ChatResponse, Message};
    use synaptic_redis::{
        LlmCache, RedisCache, RedisCacheConfig, RedisStore, RedisStoreConfig, Store,
    };

    const REDIS_URL: &str = "redis://127.0.0.1/";

    fn test_store() -> RedisStore {
        let config = RedisStoreConfig {
            prefix: "synaptic:test:store:".to_string(),
        };
        RedisStore::from_url_with_config(REDIS_URL, config).expect("Redis client creation failed")
    }

    fn test_cache() -> RedisCache {
        let config = RedisCacheConfig {
            prefix: "synaptic:test:cache:".to_string(),
            ttl: None,
        };
        RedisCache::from_url_with_config(REDIS_URL, config).expect("Redis client creation failed")
    }

    #[tokio::test]
    #[ignore = "requires running Redis"]
    async fn store_put_and_get() {
        let store = test_store();
        store
            .put(&["ns", "test"], "key1", json!("hello"))
            .await
            .unwrap();

        let item = store.get(&["ns", "test"], "key1").await.unwrap().unwrap();
        assert_eq!(item.key, "key1");
        assert_eq!(item.value, json!("hello"));
        assert_eq!(item.namespace, vec!["ns", "test"]);

        // Cleanup
        store.delete(&["ns", "test"], "key1").await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires running Redis"]
    async fn store_get_nonexistent() {
        let store = test_store();
        let item = store.get(&["ns", "missing"], "nokey").await.unwrap();
        assert!(item.is_none());
    }

    #[tokio::test]
    #[ignore = "requires running Redis"]
    async fn store_delete() {
        let store = test_store();
        store.put(&["ns", "del"], "k", json!(42)).await.unwrap();
        store.delete(&["ns", "del"], "k").await.unwrap();
        assert!(store.get(&["ns", "del"], "k").await.unwrap().is_none());
    }

    #[tokio::test]
    #[ignore = "requires running Redis"]
    async fn store_upsert_preserves_created_at() {
        let store = test_store();
        store.put(&["ns", "upsert"], "k", json!(1)).await.unwrap();
        let first = store.get(&["ns", "upsert"], "k").await.unwrap().unwrap();

        store.put(&["ns", "upsert"], "k", json!(2)).await.unwrap();
        let second = store.get(&["ns", "upsert"], "k").await.unwrap().unwrap();

        assert_eq!(first.created_at, second.created_at);
        assert_eq!(second.value, json!(2));

        // Cleanup
        store.delete(&["ns", "upsert"], "k").await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires running Redis"]
    async fn store_search_with_query() {
        let store = test_store();
        store
            .put(&["ns", "search"], "a", json!("apple pie"))
            .await
            .unwrap();
        store
            .put(&["ns", "search"], "b", json!("banana split"))
            .await
            .unwrap();
        store
            .put(&["ns", "search"], "c", json!("cherry tart"))
            .await
            .unwrap();

        let all = store.search(&["ns", "search"], None, 10).await.unwrap();
        assert_eq!(all.len(), 3);

        let filtered = store
            .search(&["ns", "search"], Some("apple"), 10)
            .await
            .unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].key, "a");

        // Cleanup
        for key in ["a", "b", "c"] {
            store.delete(&["ns", "search"], key).await.unwrap();
        }
    }

    #[tokio::test]
    #[ignore = "requires running Redis"]
    async fn store_list_namespaces() {
        let store = test_store();
        store
            .put(&["ns", "list", "a"], "k1", json!(1))
            .await
            .unwrap();
        store
            .put(&["ns", "list", "b"], "k2", json!(2))
            .await
            .unwrap();
        store.put(&["other", "ns"], "k3", json!(3)).await.unwrap();

        let all = store.list_namespaces(&[]).await.unwrap();
        // At least the 3 we just created (there may be leftover test data)
        assert!(all.len() >= 3);

        let filtered = store.list_namespaces(&["ns", "list"]).await.unwrap();
        assert!(filtered.len() >= 2);

        // Cleanup
        store.delete(&["ns", "list", "a"], "k1").await.unwrap();
        store.delete(&["ns", "list", "b"], "k2").await.unwrap();
        store.delete(&["other", "ns"], "k3").await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires running Redis"]
    async fn cache_put_and_get() {
        let cache = test_cache();
        let response = ChatResponse {
            message: Message::ai("Hello from cache"),
            usage: None,
        };

        cache.put("test_key", &response).await.unwrap();
        let cached = cache.get("test_key").await.unwrap().unwrap();
        assert_eq!(cached.message.content(), "Hello from cache");

        // Cleanup via clear
        cache.clear().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "requires running Redis"]
    async fn cache_get_nonexistent() {
        let cache = test_cache();
        let result = cache.get("nonexistent_key_12345").await.unwrap();
        assert!(result.is_none());
    }

    #[tokio::test]
    #[ignore = "requires running Redis"]
    async fn cache_clear() {
        let cache = test_cache();
        let response = ChatResponse {
            message: Message::ai("to be cleared"),
            usage: None,
        };

        cache.put("clear_key_1", &response).await.unwrap();
        cache.put("clear_key_2", &response).await.unwrap();

        cache.clear().await.unwrap();

        assert!(cache.get("clear_key_1").await.unwrap().is_none());
        assert!(cache.get("clear_key_2").await.unwrap().is_none());
    }

    #[tokio::test]
    #[ignore = "requires running Redis"]
    async fn cache_with_ttl() {
        let config = RedisCacheConfig {
            prefix: "synaptic:test:ttl:".to_string(),
            ttl: Some(1), // 1 second TTL
        };
        let cache = RedisCache::from_url_with_config(REDIS_URL, config)
            .expect("Redis client creation failed");

        let response = ChatResponse {
            message: Message::ai("expires soon"),
            usage: None,
        };

        cache.put("ttl_key", &response).await.unwrap();

        // Should exist immediately
        assert!(cache.get("ttl_key").await.unwrap().is_some());

        // Wait for TTL to expire
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        // Should be expired now
        assert!(cache.get("ttl_key").await.unwrap().is_none());
    }
}
