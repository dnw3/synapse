use synaptic_graph::{Checkpoint, CheckpointConfig, Checkpointer, MemorySaver};

#[tokio::test]
async fn memory_saver_put_get() {
    let saver = MemorySaver::new();
    let config = CheckpointConfig::new("thread-1");

    let cp = Checkpoint {
        state: serde_json::json!({"counter": 5}),
        next_node: Some("node_b".to_string()),
    };

    saver.put(&config, &cp).await.unwrap();

    let retrieved = saver.get(&config).await.unwrap().unwrap();
    assert_eq!(retrieved.state["counter"], 5);
    assert_eq!(retrieved.next_node.as_deref(), Some("node_b"));
}

#[tokio::test]
async fn memory_saver_list() {
    let saver = MemorySaver::new();
    let config = CheckpointConfig::new("thread-2");

    for i in 0..3 {
        let cp = Checkpoint {
            state: serde_json::json!({"step": i}),
            next_node: None,
        };
        saver.put(&config, &cp).await.unwrap();
    }

    let all = saver.list(&config).await.unwrap();
    assert_eq!(all.len(), 3);
    assert_eq!(all[0].state["step"], 0);
    assert_eq!(all[2].state["step"], 2);
}

#[tokio::test]
async fn memory_saver_returns_latest() {
    let saver = MemorySaver::new();
    let config = CheckpointConfig::new("thread-3");

    let cp1 = Checkpoint {
        state: serde_json::json!({"v": 1}),
        next_node: Some("a".to_string()),
    };
    let cp2 = Checkpoint {
        state: serde_json::json!({"v": 2}),
        next_node: Some("b".to_string()),
    };

    saver.put(&config, &cp1).await.unwrap();
    saver.put(&config, &cp2).await.unwrap();

    let latest = saver.get(&config).await.unwrap().unwrap();
    assert_eq!(latest.state["v"], 2);
    assert_eq!(latest.next_node.as_deref(), Some("b"));
}

#[tokio::test]
async fn memory_saver_empty_thread() {
    let saver = MemorySaver::new();
    let config = CheckpointConfig::new("nonexistent");

    let result = saver.get(&config).await.unwrap();
    assert!(result.is_none());

    let list = saver.list(&config).await.unwrap();
    assert!(list.is_empty());
}
