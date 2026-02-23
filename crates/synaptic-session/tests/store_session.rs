use std::sync::Arc;

use synaptic_core::{MemoryStore, Message};
use synaptic_graph::{Checkpoint, CheckpointConfig, Checkpointer};
use synaptic_session::SessionManager;
use synaptic_store::InMemoryStore;

fn new_manager() -> SessionManager {
    SessionManager::new(Arc::new(InMemoryStore::new()))
}

#[tokio::test]
async fn create_session() {
    let mgr = new_manager();
    let id = mgr.create_session().await.unwrap();
    assert!(!id.is_empty());

    let info = mgr.get_session(&id).await.unwrap().unwrap();
    assert_eq!(info.id, id);
}

#[tokio::test]
async fn list_sessions() {
    let mgr = new_manager();
    mgr.create_session().await.unwrap();
    mgr.create_session().await.unwrap();

    let sessions = mgr.list_sessions().await.unwrap();
    assert_eq!(sessions.len(), 2);
}

#[tokio::test]
async fn delete_session() {
    let mgr = new_manager();
    let id = mgr.create_session().await.unwrap();

    mgr.delete_session(&id).await.unwrap();

    let info = mgr.get_session(&id).await.unwrap();
    assert!(info.is_none());
}

#[tokio::test]
async fn memory_integration() {
    let mgr = new_manager();
    let id = mgr.create_session().await.unwrap();

    let memory = mgr.memory();
    memory.append(&id, Message::human("Hello")).await.unwrap();
    memory.append(&id, Message::ai("Hi there!")).await.unwrap();

    let messages = memory.load(&id).await.unwrap();
    assert_eq!(messages.len(), 2);
    assert!(messages[0].is_human());
    assert_eq!(messages[0].content(), "Hello");
    assert!(messages[1].is_ai());
    assert_eq!(messages[1].content(), "Hi there!");
}

#[tokio::test]
async fn checkpointer_integration() {
    let mgr = new_manager();
    let id = mgr.create_session().await.unwrap();

    let cp = mgr.checkpointer();
    let config = CheckpointConfig::new(&id);

    let checkpoint = Checkpoint::new(serde_json::json!({"value": "test"}), None);
    let checkpoint_id = checkpoint.id.clone();
    cp.put(&config, &checkpoint).await.unwrap();

    let loaded = cp.get(&config).await.unwrap().unwrap();
    assert_eq!(loaded.id, checkpoint_id);
    assert_eq!(loaded.state, serde_json::json!({"value": "test"}));
}

#[tokio::test]
async fn delete_session_cleans_up_all_data() {
    let mgr = new_manager();
    let id = mgr.create_session().await.unwrap();

    // Add messages
    let memory = mgr.memory();
    memory.append(&id, Message::human("Hello")).await.unwrap();

    // Add checkpoint
    let cp = mgr.checkpointer();
    let config = CheckpointConfig::new(&id);
    let checkpoint = Checkpoint::new(serde_json::json!({"v": 1}), None);
    cp.put(&config, &checkpoint).await.unwrap();

    // Delete everything
    mgr.delete_session(&id).await.unwrap();

    // Verify all data is gone
    assert!(mgr.get_session(&id).await.unwrap().is_none());
    assert!(memory.load(&id).await.unwrap().is_empty());
    assert!(cp.get(&config).await.unwrap().is_none());
}
