use std::sync::Arc;

use synaptic_core::{MemoryStore, Message};
use synaptic_memory::{ChatMessageHistory, ConversationTokenBufferMemory};

fn new_store() -> Arc<ChatMessageHistory> {
    Arc::new(ChatMessageHistory::new(Arc::new(
        synaptic_store::InMemoryStore::new(),
    )))
}

#[tokio::test]
async fn token_buffer_trims_old_messages() {
    let store = new_store();
    // Set a small token budget. "hello" = 5 chars => ~2 tokens, "world" = 5 chars => ~2 tokens
    // Each short message is about 2 tokens, so 10 tokens should fit ~5 short messages
    let token_buf = ConversationTokenBufferMemory::new(store, 5);

    token_buf
        .append("s1", Message::human("aaaa bbbb cccc dddd")) // 19 chars => ~5 tokens
        .await
        .unwrap();
    token_buf
        .append("s1", Message::ai("eeee ffff")) // 9 chars => ~3 tokens
        .await
        .unwrap();
    token_buf
        .append("s1", Message::human("gg")) // 2 chars => ~1 token
        .await
        .unwrap();

    let loaded = token_buf.load("s1").await.unwrap();
    // Total would be ~9 tokens, budget is 5
    // Should drop oldest messages until within budget
    // After dropping first (5 tokens): ~4 tokens remain => fits
    assert!(loaded.len() < 3);
    // The last message should always be present
    assert_eq!(loaded.last().unwrap().content(), "gg");
}

#[tokio::test]
async fn token_buffer_returns_all_when_under_limit() {
    let store = new_store();
    let token_buf = ConversationTokenBufferMemory::new(store, 100);

    token_buf
        .append("s1", Message::human("hello"))
        .await
        .unwrap();
    token_buf.append("s1", Message::ai("world")).await.unwrap();

    let loaded = token_buf.load("s1").await.unwrap();
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].content(), "hello");
    assert_eq!(loaded[1].content(), "world");
}

#[tokio::test]
async fn token_buffer_clear() {
    let store = new_store();
    let token_buf = ConversationTokenBufferMemory::new(store, 100);

    token_buf
        .append("s1", Message::human("hello"))
        .await
        .unwrap();
    token_buf.clear("s1").await.unwrap();

    let loaded = token_buf.load("s1").await.unwrap();
    assert!(loaded.is_empty());
}
