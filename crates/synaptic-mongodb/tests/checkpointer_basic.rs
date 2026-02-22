// Integration test only - requires a real MongoDB instance.
// Run with: cargo test -p synaptic-mongodb -- --ignored

/// Integration test for MongoCheckpointer.
///
/// Requires a MongoDB instance at `mongodb://localhost:27017`.
/// Skip automatically in CI by marking `#[ignore]`.
#[tokio::test]
#[ignore]
async fn test_mongo_checkpointer_put_and_get() {
    use synaptic_graph::{Checkpoint, CheckpointConfig, Checkpointer};
    use synaptic_mongodb::MongoCheckpointer;

    let client = mongodb::Client::with_uri_str("mongodb://localhost:27017")
        .await
        .unwrap();
    let db = client.database("synaptic_test");
    // Use a unique collection per test run to avoid collisions
    let coll = format!("test_checkpoints_{}", uuid_v4());
    let cp = MongoCheckpointer::new(&db, &coll).await.unwrap();

    let config = CheckpointConfig::new("thread-mongo-1");
    let checkpoint = Checkpoint::new(serde_json::json!({"step": 1}), None);
    let id = checkpoint.id.clone();

    cp.put(&config, &checkpoint).await.unwrap();
    let retrieved = cp.get(&config).await.unwrap().unwrap();
    assert_eq!(retrieved.id, id);
    assert_eq!(retrieved.state["step"], serde_json::json!(1));

    // Cleanup
    db.collection::<bson::Document>(&coll).drop().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_mongo_checkpointer_list() {
    use synaptic_graph::{Checkpoint, CheckpointConfig, Checkpointer};
    use synaptic_mongodb::MongoCheckpointer;

    let client = mongodb::Client::with_uri_str("mongodb://localhost:27017")
        .await
        .unwrap();
    let db = client.database("synaptic_test");
    let coll = format!("test_checkpoints_{}", uuid_v4());
    let cp = MongoCheckpointer::new(&db, &coll).await.unwrap();

    let config = CheckpointConfig::new("thread-mongo-list");
    for i in 0..3 {
        cp.put(
            &config,
            &Checkpoint::new(serde_json::json!({"step": i}), None),
        )
        .await
        .unwrap();
    }

    let list = cp.list(&config).await.unwrap();
    assert_eq!(list.len(), 3);

    // Cleanup
    db.collection::<bson::Document>(&coll).drop().await.unwrap();
}

#[tokio::test]
#[ignore]
async fn test_mongo_checkpointer_empty_thread() {
    use synaptic_graph::{CheckpointConfig, Checkpointer};
    use synaptic_mongodb::MongoCheckpointer;

    let client = mongodb::Client::with_uri_str("mongodb://localhost:27017")
        .await
        .unwrap();
    let db = client.database("synaptic_test");
    let coll = format!("test_checkpoints_{}", uuid_v4());
    let cp = MongoCheckpointer::new(&db, &coll).await.unwrap();

    let config = CheckpointConfig::new("no-such-thread");
    assert!(cp.get(&config).await.unwrap().is_none());
    assert!(cp.list(&config).await.unwrap().is_empty());

    // Cleanup
    db.collection::<bson::Document>(&coll).drop().await.unwrap();
}

/// Simple pseudo-UUID for test collection naming (avoids a uuid dep).
fn uuid_v4() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    format!("{ts:x}")
}
