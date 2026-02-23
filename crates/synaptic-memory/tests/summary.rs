use std::sync::Arc;

use synaptic_core::{ChatResponse, MemoryStore, Message};
use synaptic_memory::{ChatMessageHistory, ConversationSummaryMemory};

fn new_store() -> Arc<ChatMessageHistory> {
    Arc::new(ChatMessageHistory::new(Arc::new(
        synaptic_store::InMemoryStore::new(),
    )))
}
use synaptic_models::ScriptedChatModel;

#[tokio::test]
async fn summary_keeps_recent_messages() {
    // Create a scripted model that returns a summary when asked
    let model = Arc::new(ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai("User greeted and asked about weather."),
        usage: None,
    }]));

    let store = new_store();
    let summary = ConversationSummaryMemory::new(store, model, 4);

    // Add messages under the threshold (buffer_size * 2 = 8)
    summary.append("s1", Message::human("hello")).await.unwrap();
    summary.append("s1", Message::ai("hi")).await.unwrap();
    summary
        .append("s1", Message::human("how are you?"))
        .await
        .unwrap();
    summary.append("s1", Message::ai("I'm good")).await.unwrap();

    let loaded = summary.load("s1").await.unwrap();
    // Should return all 4 messages with no summary (under threshold)
    assert_eq!(loaded.len(), 4);
    assert_eq!(loaded[0].content(), "hello");
    assert_eq!(loaded[3].content(), "I'm good");
}

#[tokio::test]
async fn summary_generates_summary_on_overflow() {
    // Create a scripted model that returns a summary when asked
    let model = Arc::new(ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai("User discussed greetings and weather."),
        usage: None,
    }]));

    let store = new_store();
    // buffer_size=2, so summarization triggers at > 4 messages
    let summary = ConversationSummaryMemory::new(store, model, 2);

    // Add 5 messages to trigger summarization (> buffer_size * 2 = 4)
    summary.append("s1", Message::human("hello")).await.unwrap();
    summary.append("s1", Message::ai("hi")).await.unwrap();
    summary
        .append("s1", Message::human("weather?"))
        .await
        .unwrap();
    summary.append("s1", Message::ai("sunny")).await.unwrap();
    // This 5th message triggers summarization
    summary
        .append("s1", Message::human("thanks"))
        .await
        .unwrap();

    let loaded = summary.load("s1").await.unwrap();

    // Should have: 1 summary system message + 2 recent messages (buffer_size)
    assert_eq!(loaded.len(), 3);
    assert!(loaded[0].is_system());
    assert!(loaded[0]
        .content()
        .contains("User discussed greetings and weather."));
    // The last 2 messages should be the most recent
    assert_eq!(loaded[1].content(), "sunny");
    assert_eq!(loaded[2].content(), "thanks");
}
