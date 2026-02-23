use std::sync::Arc;

use synaptic_graph::{Checkpoint, CheckpointConfig, Checkpointer, StoreCheckpointer};
use synaptic_store::InMemoryStore;

fn new_checkpointer() -> StoreCheckpointer {
    StoreCheckpointer::new(Arc::new(InMemoryStore::new()))
}

#[tokio::test]
async fn put_and_get_latest() {
    let cp = new_checkpointer();
    let config = CheckpointConfig::new("thread-1");

    let ckpt = Checkpoint::new(serde_json::json!({"count": 1}), Some("node_a".to_string()));
    let id = ckpt.id.clone();
    cp.put(&config, &ckpt).await.unwrap();

    let loaded = cp.get(&config).await.unwrap().unwrap();
    assert_eq!(loaded.id, id);
    assert_eq!(loaded.state, serde_json::json!({"count": 1}));
    assert_eq!(loaded.next_node, Some("node_a".to_string()));
}

#[tokio::test]
async fn get_returns_none_for_empty_thread() {
    let cp = new_checkpointer();
    let config = CheckpointConfig::new("nonexistent");
    assert!(cp.get(&config).await.unwrap().is_none());
}

#[tokio::test]
async fn list_returns_checkpoints_in_order() {
    let cp = new_checkpointer();
    let config = CheckpointConfig::new("thread-1");

    let ckpt1 = Checkpoint::new(serde_json::json!({"step": 1}), None);
    let id1 = ckpt1.id.clone();
    cp.put(&config, &ckpt1).await.unwrap();

    // Small delay to ensure different timestamp-based IDs
    tokio::time::sleep(std::time::Duration::from_millis(1)).await;

    let ckpt2 = Checkpoint::new(serde_json::json!({"step": 2}), None);
    let id2 = ckpt2.id.clone();
    cp.put(&config, &ckpt2).await.unwrap();

    let list = cp.list(&config).await.unwrap();
    assert_eq!(list.len(), 2);
    assert_eq!(list[0].id, id1);
    assert_eq!(list[1].id, id2);
}

#[tokio::test]
async fn get_latest_returns_most_recent() {
    let cp = new_checkpointer();
    let config = CheckpointConfig::new("thread-1");

    let ckpt1 = Checkpoint::new(serde_json::json!({"step": 1}), None);
    cp.put(&config, &ckpt1).await.unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(1)).await;

    let ckpt2 = Checkpoint::new(serde_json::json!({"step": 2}), None);
    let id2 = ckpt2.id.clone();
    cp.put(&config, &ckpt2).await.unwrap();

    let latest = cp.get(&config).await.unwrap().unwrap();
    assert_eq!(latest.id, id2);
    assert_eq!(latest.state, serde_json::json!({"step": 2}));
}

#[tokio::test]
async fn get_by_specific_checkpoint_id() {
    let cp = new_checkpointer();
    let config = CheckpointConfig::new("thread-1");

    let ckpt1 = Checkpoint::new(serde_json::json!({"step": 1}), None);
    let id1 = ckpt1.id.clone();
    cp.put(&config, &ckpt1).await.unwrap();

    tokio::time::sleep(std::time::Duration::from_millis(1)).await;

    let ckpt2 = Checkpoint::new(serde_json::json!({"step": 2}), None);
    cp.put(&config, &ckpt2).await.unwrap();

    // Fetch the first checkpoint by ID
    let specific_config = CheckpointConfig::with_checkpoint_id("thread-1", &id1);
    let loaded = cp.get(&specific_config).await.unwrap().unwrap();
    assert_eq!(loaded.id, id1);
    assert_eq!(loaded.state, serde_json::json!({"step": 1}));
}

#[tokio::test]
async fn threads_are_isolated() {
    let cp = new_checkpointer();

    let config_a = CheckpointConfig::new("thread-a");
    let config_b = CheckpointConfig::new("thread-b");

    cp.put(&config_a, &Checkpoint::new(serde_json::json!("a"), None))
        .await
        .unwrap();
    cp.put(&config_b, &Checkpoint::new(serde_json::json!("b"), None))
        .await
        .unwrap();

    let a = cp.get(&config_a).await.unwrap().unwrap();
    let b = cp.get(&config_b).await.unwrap().unwrap();

    assert_eq!(a.state, serde_json::json!("a"));
    assert_eq!(b.state, serde_json::json!("b"));

    assert_eq!(cp.list(&config_a).await.unwrap().len(), 1);
    assert_eq!(cp.list(&config_b).await.unwrap().len(), 1);
}
