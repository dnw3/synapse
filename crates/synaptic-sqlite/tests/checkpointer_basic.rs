use serde_json::json;
use synaptic_graph::{Checkpoint, CheckpointConfig, Checkpointer};
use synaptic_sqlite::SqliteCheckpointer;

#[tokio::test]
async fn test_put_and_get_checkpoint() {
    let cp = SqliteCheckpointer::in_memory().unwrap();
    let config = CheckpointConfig::new("thread-1");
    let checkpoint = Checkpoint::new(json!({"messages": ["hello"]}), Some("node_a".to_string()));
    let checkpoint_id = checkpoint.id.clone();

    cp.put(&config, &checkpoint).await.unwrap();

    let retrieved = cp.get(&config).await.unwrap().unwrap();
    assert_eq!(retrieved.id, checkpoint_id);
    assert_eq!(retrieved.next_node, Some("node_a".to_string()));
}

#[tokio::test]
async fn test_get_specific_checkpoint() {
    let cp = SqliteCheckpointer::in_memory().unwrap();
    let config = CheckpointConfig::new("thread-2");

    let cp1 = Checkpoint::new(json!({"step": 1}), None);
    let cp2 = Checkpoint::new(json!({"step": 2}), None);
    let id1 = cp1.id.clone();

    cp.put(&config, &cp1).await.unwrap();
    cp.put(&config, &cp2).await.unwrap();

    // Get specific checkpoint by ID
    let specific_config = CheckpointConfig::with_checkpoint_id("thread-2", &id1);
    let retrieved = cp.get(&specific_config).await.unwrap().unwrap();
    assert_eq!(retrieved.id, id1);
    // Verify the state matches the first checkpoint
    assert_eq!(retrieved.state["step"], json!(1));
}

#[tokio::test]
async fn test_get_latest_checkpoint() {
    let cp = SqliteCheckpointer::in_memory().unwrap();
    let config = CheckpointConfig::new("thread-latest");

    let cp1 = Checkpoint::new(json!({"step": 1}), None);
    let cp2 = Checkpoint::new(json!({"step": 2}), None);
    let id2 = cp2.id.clone();

    cp.put(&config, &cp1).await.unwrap();
    cp.put(&config, &cp2).await.unwrap();

    // Get without checkpoint_id => latest
    let retrieved = cp.get(&config).await.unwrap().unwrap();
    assert_eq!(retrieved.id, id2);
    assert_eq!(retrieved.state["step"], json!(2));
}

#[tokio::test]
async fn test_list_checkpoints() {
    let cp = SqliteCheckpointer::in_memory().unwrap();
    let config = CheckpointConfig::new("thread-3");

    for i in 0..3 {
        cp.put(&config, &Checkpoint::new(json!({"step": i}), None))
            .await
            .unwrap();
    }

    let list = cp.list(&config).await.unwrap();
    assert_eq!(list.len(), 3);
    // Ordered oldest to newest
    assert_eq!(list[0].state["step"], json!(0));
    assert_eq!(list[2].state["step"], json!(2));
}

#[tokio::test]
async fn test_get_empty_thread() {
    let cp = SqliteCheckpointer::in_memory().unwrap();
    let config = CheckpointConfig::new("nonexistent-thread");
    let result = cp.get(&config).await.unwrap();
    assert!(result.is_none());
}

#[tokio::test]
async fn test_list_empty_thread() {
    let cp = SqliteCheckpointer::in_memory().unwrap();
    let config = CheckpointConfig::new("empty-thread");
    let result = cp.list(&config).await.unwrap();
    assert!(result.is_empty());
}

#[tokio::test]
async fn test_checkpoint_with_parent_and_metadata() {
    let cp = SqliteCheckpointer::in_memory().unwrap();
    let config = CheckpointConfig::new("thread-meta");

    let parent = Checkpoint::new(json!({"step": 0}), None);
    let parent_id = parent.id.clone();
    cp.put(&config, &parent).await.unwrap();

    let child = Checkpoint::new(json!({"step": 1}), Some("next_node".to_string()))
        .with_parent(&parent_id)
        .with_metadata("node", json!("step_node"));
    let child_id = child.id.clone();
    cp.put(&config, &child).await.unwrap();

    let retrieved = cp.get(&config).await.unwrap().unwrap();
    assert_eq!(retrieved.id, child_id);
    assert_eq!(retrieved.parent_id, Some(parent_id));
    assert_eq!(retrieved.metadata["node"], json!("step_node"));
}

#[tokio::test]
async fn test_idempotent_put() {
    // Putting the same checkpoint twice should not duplicate it
    let cp = SqliteCheckpointer::in_memory().unwrap();
    let config = CheckpointConfig::new("thread-idempotent");
    let checkpoint = Checkpoint::new(json!({"v": 1}), None);

    cp.put(&config, &checkpoint).await.unwrap();
    cp.put(&config, &checkpoint).await.unwrap(); // second put same ID

    let list = cp.list(&config).await.unwrap();
    assert_eq!(list.len(), 1, "Duplicate put should not create duplicates");
}
