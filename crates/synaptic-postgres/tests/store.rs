//! Integration tests for `PgStore`.
//!
//! The `#[ignore]` tests require a running PostgreSQL instance. Set the
//! `DATABASE_URL` environment variable to the connection string before running:
//!
//! ```bash
//! DATABASE_URL=postgres://user:pass@localhost/test_db cargo test -p synaptic-postgres -- --ignored store
//! ```

use serde_json::json;
use synaptic_postgres::{PgStore, PgStoreConfig, Store};

async fn setup_store(table_name: &str) -> PgStore {
    let database_url =
        std::env::var("DATABASE_URL").expect("DATABASE_URL must be set for postgres tests");
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&database_url)
        .await
        .expect("failed to connect to PostgreSQL");

    // Drop the table first so each test starts fresh
    let drop_sql = format!("DROP TABLE IF EXISTS {table_name}");
    sqlx::query(&drop_sql)
        .execute(&pool)
        .await
        .expect("failed to drop test table");

    let config = PgStoreConfig::new(table_name);
    let store = PgStore::new(pool, config);
    store.initialize().await.expect("initialize failed");
    store
}

#[tokio::test]
#[ignore]
async fn test_put_and_get() {
    let store = setup_store("test_store_put_get").await;

    store
        .put(&["users"], "alice", json!({"name": "Alice"}))
        .await
        .unwrap();

    let item = store.get(&["users"], "alice").await.unwrap().unwrap();
    assert_eq!(item.key, "alice");
    assert_eq!(item.value, json!({"name": "Alice"}));
    assert_eq!(item.namespace, vec!["users"]);
}

#[tokio::test]
#[ignore]
async fn test_get_missing() {
    let store = setup_store("test_store_get_missing").await;
    let result = store.get(&["ns"], "nonexistent").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
#[ignore]
async fn test_delete() {
    let store = setup_store("test_store_delete").await;

    store.put(&["ns"], "key1", json!("value")).await.unwrap();
    assert!(store.get(&["ns"], "key1").await.unwrap().is_some());

    store.delete(&["ns"], "key1").await.unwrap();
    assert!(store.get(&["ns"], "key1").await.unwrap().is_none());

    // Idempotent: deleting again should not error
    store.delete(&["ns"], "key1").await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_upsert_preserves_created_at() {
    let store = setup_store("test_store_upsert_created").await;

    store.put(&["ns"], "key1", json!("first")).await.unwrap();
    let first = store.get(&["ns"], "key1").await.unwrap().unwrap();
    let created = first.created_at.clone();

    // Small delay to ensure updated_at differs
    tokio::time::sleep(std::time::Duration::from_millis(10)).await;

    store.put(&["ns"], "key1", json!("second")).await.unwrap();
    let second = store.get(&["ns"], "key1").await.unwrap().unwrap();

    assert_eq!(second.created_at, created, "created_at should be preserved");
    assert_eq!(second.value, json!("second"));
    assert_ne!(
        second.updated_at, created,
        "updated_at should differ from created_at"
    );
}

#[tokio::test]
#[ignore]
async fn test_search_without_query() {
    let store = setup_store("test_store_search_no_query").await;

    store.put(&["ns"], "a", json!("alpha")).await.unwrap();
    store.put(&["ns"], "b", json!("beta")).await.unwrap();
    store.put(&["ns"], "c", json!("gamma")).await.unwrap();

    let items = store.search(&["ns"], None, 10).await.unwrap();
    assert_eq!(items.len(), 3);
}

#[tokio::test]
#[ignore]
async fn test_search_with_query() {
    let store = setup_store("test_store_search_query").await;

    store
        .put(
            &["docs"],
            "doc1",
            json!("Rust is a systems programming language"),
        )
        .await
        .unwrap();
    store
        .put(&["docs"], "doc2", json!("Python is a scripting language"))
        .await
        .unwrap();
    store
        .put(&["docs"], "doc3", json!("JavaScript runs in the browser"))
        .await
        .unwrap();

    let results = store.search(&["docs"], Some("Rust"), 10).await.unwrap();
    assert!(!results.is_empty());
    // The Rust document should be in results
    assert!(results.iter().any(|i| i.key == "doc1"));
}

#[tokio::test]
#[ignore]
async fn test_search_limit() {
    let store = setup_store("test_store_search_limit").await;

    for i in 0..10 {
        store
            .put(&["ns"], &format!("key{i}"), json!(format!("val{i}")))
            .await
            .unwrap();
    }

    let items = store.search(&["ns"], None, 3).await.unwrap();
    assert_eq!(items.len(), 3);
}

#[tokio::test]
#[ignore]
async fn test_list_namespaces_all() {
    let store = setup_store("test_store_list_ns_all").await;

    store.put(&["users"], "a", json!(1)).await.unwrap();
    store.put(&["orders"], "b", json!(2)).await.unwrap();
    store
        .put(&["users", "profiles"], "c", json!(3))
        .await
        .unwrap();

    let mut ns = store.list_namespaces(&[]).await.unwrap();
    ns.sort();

    assert_eq!(ns.len(), 3);
    assert!(ns.contains(&vec!["users".to_string()]));
    assert!(ns.contains(&vec!["orders".to_string()]));
    assert!(ns.contains(&vec!["users".to_string(), "profiles".to_string()]));
}

#[tokio::test]
#[ignore]
async fn test_list_namespaces_with_prefix() {
    let store = setup_store("test_store_list_ns_prefix").await;

    store.put(&["users"], "a", json!(1)).await.unwrap();
    store.put(&["orders"], "b", json!(2)).await.unwrap();
    store
        .put(&["users", "profiles"], "c", json!(3))
        .await
        .unwrap();

    let ns = store.list_namespaces(&["users"]).await.unwrap();
    assert_eq!(ns.len(), 2);
    assert!(ns.contains(&vec!["users".to_string()]));
    assert!(ns.contains(&vec!["users".to_string(), "profiles".to_string()]));
}

#[tokio::test]
#[ignore]
async fn test_namespace_isolation() {
    let store = setup_store("test_store_ns_isolation").await;

    store.put(&["ns1"], "key", json!("value1")).await.unwrap();
    store.put(&["ns2"], "key", json!("value2")).await.unwrap();

    let item1 = store.get(&["ns1"], "key").await.unwrap().unwrap();
    let item2 = store.get(&["ns2"], "key").await.unwrap().unwrap();

    assert_eq!(item1.value, json!("value1"));
    assert_eq!(item2.value, json!("value2"));

    // Search in ns1 should not return ns2 items
    let results = store.search(&["ns1"], None, 10).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].value, json!("value1"));
}
