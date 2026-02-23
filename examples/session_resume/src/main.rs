use std::sync::Arc;

use synaptic::core::{MemoryStore, Message};
use synaptic::session::SessionManager;
use synaptic::store::InMemoryStore;

#[tokio::main]
async fn main() {
    println!("=== Session Lifecycle Demo ===\n");

    // Create a session manager backed by an in-memory store
    let store = Arc::new(InMemoryStore::new());
    let manager = SessionManager::new(store);

    // Create a new session
    let session_id = manager.create_session().await.unwrap();
    let info = manager.get_session(&session_id).await.unwrap().unwrap();
    println!("Created session: {}", info.id);
    println!("Created at:      {}", info.created_at);

    // Use the memory interface to append messages to the session transcript
    let memory = manager.memory();
    memory
        .append(&session_id, Message::system("You are a helpful assistant."))
        .await
        .unwrap();
    memory
        .append(&session_id, Message::human("Hello, who are you?"))
        .await
        .unwrap();
    memory
        .append(
            &session_id,
            Message::ai("I am a helpful assistant. How can I help?"),
        )
        .await
        .unwrap();
    memory
        .append(&session_id, Message::human("Tell me about Rust."))
        .await
        .unwrap();
    memory
        .append(
            &session_id,
            Message::ai("Rust is a systems programming language."),
        )
        .await
        .unwrap();

    let messages = memory.load(&session_id).await.unwrap();
    println!("Messages appended: {}", messages.len());

    // Create a second session
    let session2_id = manager.create_session().await.unwrap();
    memory
        .append(&session2_id, Message::human("Second session message."))
        .await
        .unwrap();
    println!("\nCreated second session: {}", session2_id);

    // List all sessions
    let sessions = manager.list_sessions().await.unwrap();
    println!("\n--- All Sessions ---");
    for s in &sessions {
        let count = memory.load(&s.id).await.unwrap().len();
        println!(
            "  ID: {}  Messages: {}  Created: {}",
            s.id, count, s.created_at
        );
    }

    // Resume the first session and load its messages
    let resumed = manager.get_session(&session_id).await.unwrap().unwrap();
    let messages = memory.load(&resumed.id).await.unwrap();
    println!("\n--- Resumed session {} ---", resumed.id);
    for msg in &messages {
        println!("  [{}] {}", msg.role(), msg.content());
    }

    // Append more messages to the resumed session
    memory
        .append(&resumed.id, Message::human("What about ownership?"))
        .await
        .unwrap();
    let final_count = memory.load(&resumed.id).await.unwrap().len();
    println!(
        "\nAfter appending to resumed session: {} messages",
        final_count
    );

    println!("\nDone.");
}
