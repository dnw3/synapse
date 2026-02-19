use std::sync::Arc;

use synaptic_core::{ChatResponse, MemoryStore, Message};
use synaptic_memory::{ConversationSummaryBufferMemory, InMemoryStore};
use synaptic_models::ScriptedChatModel;

fn summary_response(text: &str) -> ChatResponse {
    ChatResponse {
        message: Message::ai(text),
        usage: None,
    }
}

#[tokio::test]
async fn below_threshold_no_summary() {
    // Very high limit so no summarization
    let model = Arc::new(ScriptedChatModel::new(vec![]));
    let store = Arc::new(InMemoryStore::new());
    let memory = ConversationSummaryBufferMemory::new(store, model, 10_000);

    memory.append("s1", Message::human("hi")).await.unwrap();
    memory.append("s1", Message::ai("hello")).await.unwrap();

    let loaded = memory.load("s1").await.unwrap();
    assert_eq!(loaded.len(), 2);
    // No system message with summary
    assert!(loaded[0].is_human());
}

#[tokio::test]
async fn above_threshold_calls_model() {
    let model = Arc::new(ScriptedChatModel::new(vec![
        summary_response("Summary of conversation."),
        summary_response("Extended summary."),
        summary_response("More summary."),
    ]));
    let store = Arc::new(InMemoryStore::new());
    // Very low limit to force summarization
    let memory = ConversationSummaryBufferMemory::new(store, model, 3);

    memory.append("s1", Message::human("first")).await.unwrap();
    memory.append("s1", Message::ai("second")).await.unwrap();
    memory.append("s1", Message::human("third")).await.unwrap();
    memory.append("s1", Message::ai("fourth")).await.unwrap();

    let loaded = memory.load("s1").await.unwrap();
    // Should have a summary system message at the beginning
    assert!(loaded[0].is_system());
    assert!(loaded[0].content().contains("Summary"));
}

#[tokio::test]
async fn clear_resets_everything() {
    let model = Arc::new(ScriptedChatModel::new(vec![
        summary_response("A summary."),
        summary_response("More."),
    ]));
    let store = Arc::new(InMemoryStore::new());
    let memory = ConversationSummaryBufferMemory::new(store, model, 3);

    memory.append("s1", Message::human("a")).await.unwrap();
    memory.append("s1", Message::ai("b")).await.unwrap();
    memory.append("s1", Message::human("c")).await.unwrap();

    memory.clear("s1").await.unwrap();
    let loaded = memory.load("s1").await.unwrap();
    assert!(loaded.is_empty());
}

#[tokio::test]
async fn multiple_sessions_isolated() {
    let model = Arc::new(ScriptedChatModel::new(vec![]));
    let store = Arc::new(InMemoryStore::new());
    let memory = ConversationSummaryBufferMemory::new(store, model, 10_000);

    memory.append("s1", Message::human("session 1")).await.unwrap();
    memory.append("s2", Message::human("session 2")).await.unwrap();

    let s1 = memory.load("s1").await.unwrap();
    let s2 = memory.load("s2").await.unwrap();
    assert_eq!(s1.len(), 1);
    assert_eq!(s2.len(), 1);
    assert_eq!(s1[0].content(), "session 1");
    assert_eq!(s2[0].content(), "session 2");
}

#[tokio::test]
async fn single_message_no_crash() {
    let model = Arc::new(ScriptedChatModel::new(vec![]));
    let store = Arc::new(InMemoryStore::new());
    let memory = ConversationSummaryBufferMemory::new(store, model, 10_000);

    memory.append("s1", Message::human("only one")).await.unwrap();

    let loaded = memory.load("s1").await.unwrap();
    assert_eq!(loaded.len(), 1);
    assert_eq!(loaded[0].content(), "only one");
}
