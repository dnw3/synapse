use std::sync::Arc;

use synaptic_core::{MemoryStore, Message};
use synaptic_memory::{ConversationBufferMemory, InMemoryStore};

#[tokio::test]
async fn buffer_stores_full_conversation() {
    let store = Arc::new(InMemoryStore::new());
    let buffer = ConversationBufferMemory::new(store);

    buffer.append("s1", Message::human("hello")).await.unwrap();
    buffer.append("s1", Message::ai("hi there")).await.unwrap();
    buffer
        .append("s1", Message::human("how are you?"))
        .await
        .unwrap();
    buffer
        .append("s1", Message::ai("doing well"))
        .await
        .unwrap();

    let loaded = buffer.load("s1").await.unwrap();
    assert_eq!(loaded.len(), 4);
    assert_eq!(loaded[0].content(), "hello");
    assert_eq!(loaded[1].content(), "hi there");
    assert_eq!(loaded[2].content(), "how are you?");
    assert_eq!(loaded[3].content(), "doing well");
}

#[tokio::test]
async fn buffer_clear() {
    let store = Arc::new(InMemoryStore::new());
    let buffer = ConversationBufferMemory::new(store);

    buffer.append("s1", Message::human("hello")).await.unwrap();
    buffer.clear("s1").await.unwrap();

    let loaded = buffer.load("s1").await.unwrap();
    assert!(loaded.is_empty());
}
