use std::sync::Arc;

use synaptic_core::{MemoryStore, Message};
use synaptic_memory::{ChatMessageHistory, ConversationWindowMemory};

fn new_store() -> Arc<ChatMessageHistory> {
    Arc::new(ChatMessageHistory::new(Arc::new(
        synaptic_store::InMemoryStore::new(),
    )))
}

#[tokio::test]
async fn empty_history() {
    let store = new_store();
    let window = ConversationWindowMemory::new(store, 5);
    let loaded = window.load("empty_session").await.unwrap();
    assert!(loaded.is_empty());
}

#[tokio::test]
async fn exactly_k_messages() {
    let store = new_store();
    let window = ConversationWindowMemory::new(store, 3);

    window.append("s1", Message::human("a")).await.unwrap();
    window.append("s1", Message::ai("b")).await.unwrap();
    window.append("s1", Message::human("c")).await.unwrap();

    let loaded = window.load("s1").await.unwrap();
    assert_eq!(loaded.len(), 3);
    assert_eq!(loaded[0].content(), "a");
    assert_eq!(loaded[2].content(), "c");
}

#[tokio::test]
async fn more_than_k() {
    let store = new_store();
    let window = ConversationWindowMemory::new(store, 2);

    window.append("s1", Message::human("a")).await.unwrap();
    window.append("s1", Message::ai("b")).await.unwrap();
    window.append("s1", Message::human("c")).await.unwrap();
    window.append("s1", Message::ai("d")).await.unwrap();

    let loaded = window.load("s1").await.unwrap();
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].content(), "c");
    assert_eq!(loaded[1].content(), "d");
}

#[tokio::test]
async fn k_equals_one() {
    let store = new_store();
    let window = ConversationWindowMemory::new(store, 1);

    window.append("s1", Message::human("first")).await.unwrap();
    window.append("s1", Message::ai("second")).await.unwrap();
    window.append("s1", Message::human("third")).await.unwrap();

    let loaded = window.load("s1").await.unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].content(), "third");
}

#[tokio::test]
async fn multiple_sessions_isolated() {
    let store = new_store();
    let window = ConversationWindowMemory::new(store, 5);

    window
        .append("s1", Message::human("hello from s1"))
        .await
        .unwrap();
    window
        .append("s2", Message::human("hello from s2"))
        .await
        .unwrap();

    let s1 = window.load("s1").await.unwrap();
    let s2 = window.load("s2").await.unwrap();
    assert_eq!(s1.len(), 1);
    assert_eq!(s2.len(), 1);
    assert_eq!(s1[0].content(), "hello from s1");
    assert_eq!(s2[0].content(), "hello from s2");
}

#[tokio::test]
async fn clear_one_session_preserves_other() {
    let store = new_store();
    let window = ConversationWindowMemory::new(store, 5);

    window.append("s1", Message::human("keep")).await.unwrap();
    window.append("s2", Message::human("remove")).await.unwrap();
    window.clear("s2").await.unwrap();

    let s1 = window.load("s1").await.unwrap();
    let s2 = window.load("s2").await.unwrap();
    assert_eq!(s1.len(), 1);
    assert!(s2.is_empty());
}
