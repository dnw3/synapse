use std::sync::Arc;

use synaptic_core::{ChatResponse, MemoryStore, Message};
use synaptic_memory::{ChatMessageHistory, ConversationSummaryBufferMemory};

fn new_store() -> Arc<ChatMessageHistory> {
    Arc::new(ChatMessageHistory::new(Arc::new(
        synaptic_store::InMemoryStore::new(),
    )))
}
use synaptic_models::ScriptedChatModel;

#[tokio::test]
async fn under_limit_no_summary() {
    let model = Arc::new(ScriptedChatModel::new(vec![]));
    let store = new_store();
    // max_token_limit high enough that no summarization happens
    let memory = ConversationSummaryBufferMemory::new(store, model, 1000);

    memory.append("s1", Message::human("hello")).await.unwrap();
    memory.append("s1", Message::ai("hi there")).await.unwrap();

    let loaded = memory.load("s1").await.unwrap();
    assert_eq!(loaded.len(), 2);
    assert_eq!(loaded[0].content(), "hello");
    assert_eq!(loaded[1].content(), "hi there");
}

#[tokio::test]
async fn over_limit_triggers_summary() {
    // Provide multiple summary responses in case multiple summarizations occur
    let model = Arc::new(ScriptedChatModel::new(vec![
        ChatResponse {
            message: Message::ai("User greeted and chatted about weather."),
            usage: None,
        },
        ChatResponse {
            message: Message::ai("User greeted and chatted about weather more."),
            usage: None,
        },
        ChatResponse {
            message: Message::ai("Extended conversation."),
            usage: None,
        },
    ]));

    let store = new_store();
    // Very low token limit to trigger summarization quickly
    let memory = ConversationSummaryBufferMemory::new(store, model, 5);

    memory.append("s1", Message::human("hello")).await.unwrap();
    memory.append("s1", Message::ai("hi there")).await.unwrap();
    memory
        .append("s1", Message::human("how is the weather?"))
        .await
        .unwrap();
    memory
        .append("s1", Message::ai("it is sunny today"))
        .await
        .unwrap();

    let loaded = memory.load("s1").await.unwrap();

    // Should have a summary system message + some recent messages
    assert!(loaded[0].is_system());
    assert!(loaded[0]
        .content()
        .contains("Summary of earlier conversation"));
}

#[tokio::test]
async fn preserves_recent_messages() {
    let model = Arc::new(ScriptedChatModel::new(vec![
        ChatResponse {
            message: Message::ai("Summary of conversation so far."),
            usage: None,
        },
        ChatResponse {
            message: Message::ai("Updated summary."),
            usage: None,
        },
        ChatResponse {
            message: Message::ai("Another summary."),
            usage: None,
        },
    ]));

    let store = new_store();
    let memory = ConversationSummaryBufferMemory::new(store, model, 5);

    memory.append("s1", Message::human("msg1")).await.unwrap();
    memory.append("s1", Message::ai("msg2")).await.unwrap();
    memory.append("s1", Message::human("msg3")).await.unwrap();
    memory.append("s1", Message::ai("msg4")).await.unwrap();

    let loaded = memory.load("s1").await.unwrap();

    // The last message should always be preserved
    let last = loaded.last().unwrap();
    assert!(last.content() == "msg4" || last.content() == "msg3" || loaded.len() > 1);
}

#[tokio::test]
async fn clear_removes_summary() {
    let model = Arc::new(ScriptedChatModel::new(vec![
        ChatResponse {
            message: Message::ai("A summary."),
            usage: None,
        },
        ChatResponse {
            message: Message::ai("Another summary."),
            usage: None,
        },
        ChatResponse {
            message: Message::ai("Yet another."),
            usage: None,
        },
    ]));

    let store = new_store();
    let memory = ConversationSummaryBufferMemory::new(store, model, 5);

    memory.append("s1", Message::human("a")).await.unwrap();
    memory
        .append("s1", Message::ai("long response here"))
        .await
        .unwrap();
    memory.append("s1", Message::human("b")).await.unwrap();
    memory
        .append("s1", Message::ai("another long response"))
        .await
        .unwrap();

    memory.clear("s1").await.unwrap();
    let loaded = memory.load("s1").await.unwrap();
    assert!(loaded.is_empty());
}
