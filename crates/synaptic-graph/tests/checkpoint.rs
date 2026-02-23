use std::sync::Arc;
use synaptic_graph::{Checkpoint, CheckpointConfig, Checkpointer, StoreCheckpointer};

#[tokio::test]
async fn memory_saver_put_get() {
    let saver = StoreCheckpointer::new(Arc::new(synaptic_store::InMemoryStore::new()));
    let config = CheckpointConfig::new("thread-1");

    let cp = Checkpoint::new(
        serde_json::json!({"counter": 5}),
        Some("node_b".to_string()),
    );

    saver.put(&config, &cp).await.unwrap();

    let retrieved = saver.get(&config).await.unwrap().unwrap();
    assert_eq!(retrieved.state["counter"], 5);
    assert_eq!(retrieved.next_node.as_deref(), Some("node_b"));
    // New fields should be populated
    assert!(!retrieved.id.is_empty());
    assert!(retrieved.parent_id.is_none());
}

#[tokio::test]
async fn memory_saver_list() {
    let saver = StoreCheckpointer::new(Arc::new(synaptic_store::InMemoryStore::new()));
    let config = CheckpointConfig::new("thread-2");

    for i in 0..3 {
        let cp = Checkpoint::new(serde_json::json!({"step": i}), None);
        saver.put(&config, &cp).await.unwrap();
    }

    let all = saver.list(&config).await.unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].state["step"], 0);
    assert_eq!(all[2].state["step"], 2);
    // Each should have a unique ID
    assert_ne!(all[0].id, all[1].id);
    assert_ne!(all[1].id, all[2].id);
}

#[tokio::test]
async fn memory_saver_returns_latest() {
    let saver = StoreCheckpointer::new(Arc::new(synaptic_store::InMemoryStore::new()));
    let config = CheckpointConfig::new("thread-3");

    let cp1 = Checkpoint::new(serde_json::json!({"v": 1}), Some("a".to_string()));
    let cp2 = Checkpoint::new(serde_json::json!({"v": 2}), Some("b".to_string()));

    saver.put(&config, &cp1).await.unwrap();
    saver.put(&config, &cp2).await.unwrap();

    let latest = saver.get(&config).await.unwrap().unwrap();
    assert_eq!(latest.state["v"], 2);
    assert_eq!(latest.next_node.as_deref(), Some("b"));
}

#[tokio::test]
async fn memory_saver_empty_thread() {
    let saver = StoreCheckpointer::new(Arc::new(synaptic_store::InMemoryStore::new()));
    let config = CheckpointConfig::new("nonexistent");

    let result = saver.get(&config).await.unwrap();
    assert!(result.is_none());

    let list = saver.list(&config).await.unwrap();
    assert!(list.is_empty());
}

#[tokio::test]
async fn checkpoint_with_metadata() {
    let cp = Checkpoint::new(serde_json::json!({"x": 1}), Some("next".to_string()))
        .with_metadata("source", serde_json::json!("test_node"))
        .with_parent("parent-123");

    assert_eq!(cp.parent_id.as_deref(), Some("parent-123"));
    assert_eq!(cp.metadata["source"], "test_node");
    assert!(!cp.id.is_empty());
}

#[tokio::test]
async fn get_by_checkpoint_id() {
    let saver = StoreCheckpointer::new(Arc::new(synaptic_store::InMemoryStore::new()));
    let config = CheckpointConfig::new("thread-travel");

    // Save two checkpoints
    let cp1 = Checkpoint::new(serde_json::json!({"v": 1}), Some("a".to_string()));
    let cp1_id = cp1.id.clone();
    saver.put(&config, &cp1).await.unwrap();

    let cp2 = Checkpoint::new(serde_json::json!({"v": 2}), Some("b".to_string()));
    saver.put(&config, &cp2).await.unwrap();

    // Default get returns latest (v=2)
    let latest = saver.get(&config).await.unwrap().unwrap();
    assert_eq!(latest.state["v"], 2);

    // Get by specific checkpoint_id returns the older one (v=1)
    let time_travel_config = CheckpointConfig::with_checkpoint_id("thread-travel", &cp1_id);
    let old = saver.get(&time_travel_config).await.unwrap().unwrap();
    assert_eq!(old.state["v"], 1);
    assert_eq!(old.id, cp1_id);
}
