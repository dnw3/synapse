use std::sync::Arc;

use serde_json::json;
use synaptic::core::{
    ChatModel, ChatRequest, ChatResponse, MemoryStore, Message, RunnableConfig, SynapticError,
};
use synaptic::memory::{
    ConversationBufferMemory, ConversationWindowMemory, InMemoryStore, RunnableWithMessageHistory,
};
use synaptic::models::ScriptedChatModel;
use synaptic::runnables::{Runnable, RunnableLambda};

#[tokio::main]
async fn main() -> Result<(), SynapticError> {
    // --- Buffer Memory (keeps all messages) ---
    println!("=== ConversationBufferMemory ===");
    let store = Arc::new(InMemoryStore::new());
    let buffer = ConversationBufferMemory::new(store.clone());

    buffer.append("session1", Message::human("Hello")).await?;
    buffer.append("session1", Message::ai("Hi there!")).await?;
    buffer
        .append("session1", Message::human("How are you?"))
        .await?;
    buffer
        .append("session1", Message::ai("I'm doing well!"))
        .await?;

    let history = buffer.load("session1").await?;
    println!("Buffer memory ({} messages):", history.len());
    for msg in &history {
        println!("  [{}] {}", msg.role(), msg.content());
    }

    // --- Window Memory (keeps last K messages) ---
    println!("\n=== ConversationWindowMemory (window=2) ===");
    let store2 = Arc::new(InMemoryStore::new());
    let window = ConversationWindowMemory::new(store2.clone(), 2);

    window
        .append("session1", Message::human("Message 1"))
        .await?;
    window.append("session1", Message::ai("Reply 1")).await?;
    window
        .append("session1", Message::human("Message 2"))
        .await?;
    window.append("session1", Message::ai("Reply 2")).await?;
    window
        .append("session1", Message::human("Message 3"))
        .await?;

    let history = window.load("session1").await?;
    println!("Window memory ({} of 5 messages):", history.len());
    for msg in &history {
        println!("  [{}] {}", msg.role(), msg.content());
    }

    // --- RunnableWithMessageHistory ---
    println!("\n=== RunnableWithMessageHistory ===");
    let model = ScriptedChatModel::new(vec![
        ChatResponse {
            message: Message::ai("Hello! How can I help?"),
            usage: None,
        },
        ChatResponse {
            message: Message::ai("Rust is a systems programming language."),
            usage: None,
        },
    ]);

    let model_clone = model.clone();
    let inner = RunnableLambda::new(move |messages: Vec<Message>| {
        let m = model_clone.clone();
        async move {
            let request = ChatRequest::new(messages);
            let response = m.chat(request).await?;
            Ok(response.message.content().to_string())
        }
    });

    let memory_store: Arc<dyn MemoryStore> = Arc::new(InMemoryStore::new());
    let with_history = RunnableWithMessageHistory::new(inner.boxed(), memory_store.clone());

    let config = RunnableConfig::default().with_metadata("session_id", json!("demo-session"));

    let reply1 = with_history
        .invoke("Hi there!".to_string(), &config)
        .await?;
    println!("Turn 1: {reply1}");

    let reply2 = with_history
        .invoke("What is Rust?".to_string(), &config)
        .await?;
    println!("Turn 2: {reply2}");

    let saved = memory_store.load("demo-session").await?;
    println!("Saved messages: {}", saved.len());
    for msg in &saved {
        println!("  [{}] {}", msg.role(), msg.content());
    }

    println!("\nMemory strategy demo completed successfully!");
    Ok(())
}
