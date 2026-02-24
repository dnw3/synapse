use serde_json::json;
use synaptic_core::Store;
use synaptic_sqlite::{SqliteStore, SqliteStoreConfig};

#[test]
fn config_builder() {
    let config = SqliteStoreConfig::new("/tmp/test.db");
    assert_eq!(config.path, "/tmp/test.db");
}

#[test]
fn config_in_memory() {
    let config = SqliteStoreConfig::in_memory();
    assert_eq!(config.path, ":memory:");
}

#[tokio::test]
async fn put_and_get() {
    let store = SqliteStore::new(SqliteStoreConfig::in_memory()).unwrap();

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
async fn get_missing_key() {
    let store = SqliteStore::new(SqliteStoreConfig::in_memory()).unwrap();
    let result = store.get(&["ns"], "nonexistent").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn delete_and_idempotent() {
    let store = SqliteStore::new(SqliteStoreConfig::in_memory()).unwrap();

    store.put(&["ns"], "key1", json!("value")).await.unwrap();
    assert!(store.get(&["ns"], "key1").await.unwrap().is_some());

    store.delete(&["ns"], "key1").await.unwrap();
    assert!(store.get(&["ns"], "key1").await.unwrap().is_none());

    // Idempotent: deleting again should not error
    store.delete(&["ns"], "key1").await.unwrap();
}

#[tokio::test]
async fn upsert_preserves_created_at() {
    let store = SqliteStore::new(SqliteStoreConfig::in_memory()).unwrap();

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
async fn search_no_query_lists_all() {
    let store = SqliteStore::new(SqliteStoreConfig::in_memory()).unwrap();

    store.put(&["ns"], "a", json!("alpha")).await.unwrap();
    store.put(&["ns"], "b", json!("beta")).await.unwrap();
    store.put(&["ns"], "c", json!("gamma")).await.unwrap();

    let items = store.search(&["ns"], None, 10).await.unwrap();
    assert_eq!(items.len(), 3);
}

#[tokio::test]
async fn search_with_fts_query() {
    let store = SqliteStore::new(SqliteStoreConfig::in_memory()).unwrap();

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
async fn search_respects_limit() {
    let store = SqliteStore::new(SqliteStoreConfig::in_memory()).unwrap();

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
async fn list_namespaces_all() {
    let store = SqliteStore::new(SqliteStoreConfig::in_memory()).unwrap();

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
async fn list_namespaces_with_prefix() {
    let store = SqliteStore::new(SqliteStoreConfig::in_memory()).unwrap();

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
async fn namespace_isolation() {
    let store = SqliteStore::new(SqliteStoreConfig::in_memory()).unwrap();

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
