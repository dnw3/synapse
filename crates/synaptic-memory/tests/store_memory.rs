use std::sync::Arc;

use synaptic_core::{MemoryStore, Message};
use synaptic_memory::ChatMessageHistory;
use synaptic_store::InMemoryStore;

fn new_store() -> ChatMessageHistory {
    ChatMessageHistory::new(Arc::new(InMemoryStore::new()))
}

#[tokio::test]
async fn stores_and_reads_messages_by_session() {
    let store = new_store();
    let msg = Message::human("hello");

    store.append("session-a", msg.clone()).await.unwrap();

    let loaded = store.load("session-a").await.unwrap();
    assert_eq!(loaded, vec![msg]);
}

#[tokio::test]
async fn isolates_sessions() {
    let store = new_store();
    store
        .append("session-a", Message::human("A"))
        .await
        .unwrap();
    store
        .append("session-b", Message::human("B"))
        .await
        .unwrap();

    let a = store.load("session-a").await.unwrap();
    let b = store.load("session-b").await.unwrap();

    assert_eq!(a[0].content(), "A");
    assert_eq!(b[0].content(), "B");
}

#[tokio::test]
async fn clear_removes_messages_and_summary() {
    let store = new_store();
    store.append("s1", Message::human("hello")).await.unwrap();
    store.set_summary("s1", "a summary").await.unwrap();

    store.clear("s1").await.unwrap();

    let loaded = store.load("s1").await.unwrap();
    assert!(loaded.is_empty());

    let summary = store.get_summary("s1").await.unwrap();
    assert!(summary.is_none());
}

#[tokio::test]
async fn load_empty_session_returns_empty_vec() {
    let store = new_store();
    let loaded = store.load("nonexistent").await.unwrap();
    assert!(loaded.is_empty());
}

#[tokio::test]
async fn summary_round_trip() {
    let store = new_store();

    // No summary initially
    assert!(store.get_summary("s1").await.unwrap().is_none());

    store.set_summary("s1", "A brief summary").await.unwrap();
    let summary = store.get_summary("s1").await.unwrap().unwrap();
    assert_eq!(summary, "A brief summary");

    // Update summary
    store.set_summary("s1", "Updated summary").await.unwrap();
    let summary = store.get_summary("s1").await.unwrap().unwrap();
    assert_eq!(summary, "Updated summary");
}

#[tokio::test]
async fn preserves_full_message_fidelity() {
    let store = new_store();

    // Test that AI messages with tool calls survive round-trip
    let tool_call = synaptic_core::ToolCall {
        id: "tc_1".to_string(),
        name: "get_weather".to_string(),
        arguments: serde_json::json!({"city": "NYC"}),
    };
    let ai_msg = Message::ai_with_tool_calls("Let me check", vec![tool_call.clone()]);
    let tool_msg = Message::tool("Sunny, 72F", "tc_1");

    store
        .append("s1", Message::system("You are helpful"))
        .await
        .unwrap();
    store
        .append("s1", Message::human("What's the weather?"))
        .await
        .unwrap();
    store.append("s1", ai_msg).await.unwrap();
    store.append("s1", tool_msg).await.unwrap();

    let loaded = store.load("s1").await.unwrap();
    assert_eq!(loaded.len(), 4);
    assert!(loaded[0].is_system());
    assert!(loaded[1].is_human());
    assert!(loaded[2].is_ai());
    assert_eq!(loaded[2].tool_calls().len(), 1);
    assert_eq!(loaded[2].tool_calls()[0].name, "get_weather");
    assert!(loaded[3].is_tool());
    assert_eq!(loaded[3].tool_call_id().unwrap(), "tc_1");
}
