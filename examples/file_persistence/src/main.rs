use std::sync::Arc;

use serde_json::json;
use synaptic::core::Store;
use synaptic::graph::{Checkpoint, CheckpointConfig, Checkpointer, StoreCheckpointer};
use synaptic::store::FileStore;

#[tokio::main]
async fn main() {
    println!("=== File Persistence Demo ===\n");

    let tmp = std::env::temp_dir().join("synaptic_file_demo");
    let _ = std::fs::remove_dir_all(&tmp);

    // --- FileStore: key-value storage on disk ---
    println!("--- FileStore ---");
    let store = FileStore::new(tmp.join("store"));

    // Put items into namespaces
    store
        .put(&["users", "prefs"], "theme", json!("dark"))
        .await
        .unwrap();
    store
        .put(&["users", "prefs"], "language", json!("en"))
        .await
        .unwrap();
    store
        .put(
            &["agents", "memory"],
            "summary",
            json!("User prefers Rust."),
        )
        .await
        .unwrap();

    // Retrieve by key
    let item = store
        .get(&["users", "prefs"], "theme")
        .await
        .unwrap()
        .unwrap();
    println!("  get users/prefs/theme: {:?}", item.value);

    // Search within a namespace
    let results = store.search(&["users", "prefs"], None, 10).await.unwrap();
    println!("  search users/prefs (all): {} items", results.len());
    for r in &results {
        println!("    {} = {}", r.key, r.value);
    }

    // List namespaces
    let namespaces = store.list_namespaces(&[]).await.unwrap();
    println!("  namespaces: {:?}", namespaces);

    // --- StoreCheckpointer: graph checkpoint persistence ---
    println!("\n--- StoreCheckpointer (Graph Checkpoints) ---");
    let saver = StoreCheckpointer::new(Arc::new(FileStore::new(tmp.join("checkpoints"))));
    let config = CheckpointConfig::new("thread-1");

    // Save checkpoints at different steps
    let cp1 = Checkpoint::new(json!({"messages": ["Hello"]}), Some("agent".to_string()))
        .with_metadata("step", json!(1));
    saver.put(&config, &cp1).await.unwrap();
    println!("  Saved checkpoint 1: {}", cp1.id);

    let cp2 = Checkpoint::new(
        json!({"messages": ["Hello", "World"]}),
        Some("tools".to_string()),
    )
    .with_parent(&cp1.id)
    .with_metadata("step", json!(2));
    saver.put(&config, &cp2).await.unwrap();
    println!("  Saved checkpoint 2: {}", cp2.id);

    // Get latest checkpoint
    let latest = saver.get(&config).await.unwrap().unwrap();
    println!(
        "  Latest checkpoint: {} (next_node: {:?})",
        latest.id, latest.next_node
    );

    // List all checkpoints for the thread
    let all = saver.list(&config).await.unwrap();
    println!("  Total checkpoints for thread-1: {}", all.len());
    for cp in &all {
        println!(
            "    {} -> next: {:?}, parent: {:?}",
            cp.id, cp.next_node, cp.parent_id
        );
    }

    // Get a specific checkpoint by ID
    let specific_config = CheckpointConfig::with_checkpoint_id("thread-1", &cp1.id);
    let specific = saver.get(&specific_config).await.unwrap().unwrap();
    println!(
        "  Retrieved checkpoint by ID: {} (step: {:?})",
        specific.id,
        specific.metadata.get("step")
    );

    // Clean up
    let _ = std::fs::remove_dir_all(&tmp);
    println!("\nDone.");
}
