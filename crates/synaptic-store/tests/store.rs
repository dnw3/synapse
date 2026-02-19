use serde_json::json;
use synaptic_core::Store;
use synaptic_store::InMemoryStore;

// ---------------------------------------------------------------------------
// Basic CRUD
// ---------------------------------------------------------------------------

#[tokio::test]
async fn put_and_get_returns_stored_value() {
    let store = InMemoryStore::new();
    store
        .put(&["users", "settings"], "theme", json!("dark"))
        .await
        .unwrap();

    let item = store
        .get(&["users", "settings"], "theme")
        .await
        .unwrap()
        .expect("item should exist");

    assert_eq!(item.key, "theme");
    assert_eq!(item.value, json!("dark"));
    assert_eq!(item.namespace, vec!["users", "settings"]);
}

#[tokio::test]
async fn get_nonexistent_key_returns_none() {
    let store = InMemoryStore::new();
    let result = store.get(&["ns"], "missing").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn get_from_nonexistent_namespace_returns_none() {
    let store = InMemoryStore::new();
    let result = store.get(&["no", "such", "namespace"], "key").await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn delete_removes_item() {
    let store = InMemoryStore::new();
    store.put(&["ns"], "k", json!(42)).await.unwrap();
    assert!(store.get(&["ns"], "k").await.unwrap().is_some());

    store.delete(&["ns"], "k").await.unwrap();
    assert!(store.get(&["ns"], "k").await.unwrap().is_none());
}

#[tokio::test]
async fn delete_nonexistent_key_is_noop() {
    let store = InMemoryStore::new();
    // Should not error when deleting something that does not exist
    let result = store.delete(&["ns"], "ghost").await;
    assert!(result.is_ok());
}

// ---------------------------------------------------------------------------
// Upsert behavior
// ---------------------------------------------------------------------------

#[tokio::test]
async fn upsert_updates_value_and_preserves_created_at() {
    let store = InMemoryStore::new();
    store.put(&["ns"], "k", json!("v1")).await.unwrap();
    let first = store.get(&["ns"], "k").await.unwrap().unwrap();

    store.put(&["ns"], "k", json!("v2")).await.unwrap();
    let second = store.get(&["ns"], "k").await.unwrap().unwrap();

    assert_eq!(second.value, json!("v2"));
    assert_eq!(first.created_at, second.created_at, "created_at should be preserved on upsert");
}

// ---------------------------------------------------------------------------
// Search
// ---------------------------------------------------------------------------

#[tokio::test]
async fn search_without_query_returns_all_items() {
    let store = InMemoryStore::new();
    store.put(&["fruits"], "a", json!("apple")).await.unwrap();
    store.put(&["fruits"], "b", json!("banana")).await.unwrap();
    store.put(&["fruits"], "c", json!("cherry")).await.unwrap();

    let results = store.search(&["fruits"], None, 100).await.unwrap();
    assert_eq!(results.len(), 3);
}

#[tokio::test]
async fn search_with_query_filters_by_substring() {
    let store = InMemoryStore::new();
    store.put(&["items"], "a", json!("apple pie")).await.unwrap();
    store
        .put(&["items"], "b", json!("banana split"))
        .await
        .unwrap();
    store.put(&["items"], "c", json!("cherry tart")).await.unwrap();

    let results = store.search(&["items"], Some("apple"), 10).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].value, json!("apple pie"));
}

#[tokio::test]
async fn search_respects_limit() {
    let store = InMemoryStore::new();
    for i in 0..10 {
        store
            .put(&["ns"], &format!("k{}", i), json!(format!("item{}", i)))
            .await
            .unwrap();
    }

    let results = store.search(&["ns"], None, 3).await.unwrap();
    assert!(
        results.len() <= 3,
        "search should respect limit, got {} results",
        results.len()
    );
}

#[tokio::test]
async fn search_empty_namespace_returns_empty() {
    let store = InMemoryStore::new();
    let results = store.search(&["empty"], None, 10).await.unwrap();
    assert!(results.is_empty());
}

// ---------------------------------------------------------------------------
// Namespace listing
// ---------------------------------------------------------------------------

#[tokio::test]
async fn list_namespaces_returns_all_when_no_prefix() {
    let store = InMemoryStore::new();
    store.put(&["a", "b"], "k1", json!(1)).await.unwrap();
    store.put(&["a", "c"], "k2", json!(2)).await.unwrap();
    store.put(&["x", "y"], "k3", json!(3)).await.unwrap();

    let namespaces = store.list_namespaces(&[]).await.unwrap();
    assert_eq!(namespaces.len(), 3);
}

#[tokio::test]
async fn list_namespaces_with_prefix_filters() {
    let store = InMemoryStore::new();
    store.put(&["app", "users"], "k1", json!(1)).await.unwrap();
    store.put(&["app", "config"], "k2", json!(2)).await.unwrap();
    store.put(&["system", "logs"], "k3", json!(3)).await.unwrap();

    let filtered = store.list_namespaces(&["app"]).await.unwrap();
    assert_eq!(filtered.len(), 2);

    // All returned namespaces should start with "app"
    for ns in &filtered {
        assert_eq!(ns[0], "app");
    }
}

// ---------------------------------------------------------------------------
// Cross-namespace isolation
// ---------------------------------------------------------------------------

#[tokio::test]
async fn items_in_different_namespaces_are_isolated() {
    let store = InMemoryStore::new();
    store.put(&["ns1"], "key", json!("value1")).await.unwrap();
    store.put(&["ns2"], "key", json!("value2")).await.unwrap();

    let item1 = store.get(&["ns1"], "key").await.unwrap().unwrap();
    let item2 = store.get(&["ns2"], "key").await.unwrap().unwrap();

    assert_eq!(item1.value, json!("value1"));
    assert_eq!(item2.value, json!("value2"));

    // Deleting from one namespace should not affect the other
    store.delete(&["ns1"], "key").await.unwrap();
    assert!(store.get(&["ns1"], "key").await.unwrap().is_none());
    assert!(store.get(&["ns2"], "key").await.unwrap().is_some());
}

#[tokio::test]
async fn search_does_not_cross_namespaces() {
    let store = InMemoryStore::new();
    store
        .put(&["ns1"], "a", json!("shared content"))
        .await
        .unwrap();
    store
        .put(&["ns2"], "b", json!("shared content"))
        .await
        .unwrap();

    let results = store.search(&["ns1"], None, 100).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].key, "a");
}
