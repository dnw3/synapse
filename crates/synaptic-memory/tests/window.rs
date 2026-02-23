use std::sync::Arc;

use synaptic_core::{MemoryStore, Message};
use synaptic_memory::{ChatMessageHistory, ConversationWindowMemory};

fn new_store() -> Arc<ChatMessageHistory> {
    Arc::new(ChatMessageHistory::new(Arc::new(
        synaptic_store::InMemoryStore::new(),
    )))
}

#[tokio::test]
async fn window_returns_last_n_messages() {
    let store = new_store();
    let window = ConversationWindowMemory::new(store, 4);

    // Append 6 messages
    window.append("s1", Message::human("msg1")).await.unwrap();
    window.append("s1", Message::ai("msg2")).await.unwrap();
    window.append("s1", Message::human("msg3")).await.unwrap();
    window.append("s1", Message::ai("msg4")).await.unwrap();
    window.append("s1", Message::human("msg5")).await.unwrap();
    window.append("s1", Message::ai("msg6")).await.unwrap();

    let loaded = window.load("s1").await.unwrap();
    assert_eq!(loaded.len(), 4);
    assert_eq!(loaded[0].content(), "msg3");
    assert_eq!(loaded[1].content(), "msg4");
    assert_eq!(loaded[2].content(), "msg5");
    assert_eq!(loaded[3].content(), "msg6");
}

#[tokio::test]
async fn window_returns_all_when_under_limit() {
    let store = new_store();
    let window = ConversationWindowMemory::new(store, 10);

    window.append("s1", Message::human("hello")).await.unwrap();
    window.append("s1", Message::ai("world")).await.unwrap();

    let loaded = window.load("s1").await.unwrap();
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].content(), "hello");
    assert_eq!(loaded[1].content(), "world");
}

#[tokio::test]
async fn window_clear() {
    let store = new_store();
    let window = ConversationWindowMemory::new(store, 4);

    window.append("s1", Message::human("hello")).await.unwrap();
    window.clear("s1").await.unwrap();

    let loaded = window.load("s1").await.unwrap();
    assert!(loaded.is_empty());
}
