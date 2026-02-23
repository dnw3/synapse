#![cfg(feature = "filesystem")]

use serde_json::json;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use synaptic_core::Store;
use synaptic_store::FileStore;

static COUNTER: AtomicU64 = AtomicU64::new(0);

fn temp_dir(test_name: &str) -> PathBuf {
    let cnt = COUNTER.fetch_add(1, Ordering::SeqCst);
    let pid = std::process::id();
    let dir = std::env::temp_dir().join(format!("synaptic_fs_test_{}_{}_{}", test_name, pid, cnt));
    // Clean up any stale directory from a previous run
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[tokio::test]
async fn put_get() {
    let dir = temp_dir("put_get");
    let store = FileStore::new(&dir);

    store
        .put(&["users"], "alice", json!({"name": "Alice"}))
        .await
        .unwrap();
    let item = store.get(&["users"], "alice").await.unwrap().unwrap();
    assert_eq!(item.key, "alice");
    assert_eq!(item.value, json!({"name": "Alice"}));
    assert_eq!(item.namespace, vec!["users"]);

    // Cleanup
    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn delete() {
    let dir = temp_dir("delete");
    let store = FileStore::new(&dir);

    store.put(&["ns"], "k", json!(1)).await.unwrap();
    store.delete(&["ns"], "k").await.unwrap();
    assert!(store.get(&["ns"], "k").await.unwrap().is_none());

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn search_substring() {
    let dir = temp_dir("search_substring");
    let store = FileStore::new(&dir);

    store.put(&["ns"], "a", json!("apple")).await.unwrap();
    store.put(&["ns"], "b", json!("banana")).await.unwrap();
    store.put(&["ns"], "c", json!("cherry")).await.unwrap();

    let results = store.search(&["ns"], Some("apple"), 10).await.unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].key, "a");

    let all = store.search(&["ns"], None, 10).await.unwrap();
    assert_eq!(all.len(), 3);

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn list_namespaces() {
    let dir = temp_dir("list_namespaces");
    let store = FileStore::new(&dir);

    store.put(&["a", "b"], "k1", json!(1)).await.unwrap();
    store.put(&["a", "c"], "k2", json!(2)).await.unwrap();
    store.put(&["x", "y"], "k3", json!(3)).await.unwrap();

    let all = store.list_namespaces(&[]).await.unwrap();
    assert_eq!(all.len(), 3);

    std::fs::remove_dir_all(&dir).ok();
}

#[tokio::test]
async fn get_nonexistent_none() {
    let dir = temp_dir("get_nonexistent");
    let store = FileStore::new(&dir);
    assert!(store.get(&["ns"], "missing").await.unwrap().is_none());
    std::fs::remove_dir_all(&dir).ok();
}
