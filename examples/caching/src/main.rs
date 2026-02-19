use std::sync::Arc;

use synaptic::cache::{CachedChatModel, InMemoryCache, LlmCache};
use synaptic::core::{ChatModel, ChatRequest, ChatResponse, Message, SynapticError};
use synaptic::models::ScriptedChatModel;

#[tokio::main]
async fn main() -> Result<(), SynapticError> {
    // --- Setup: model with limited responses + cache ---
    // ScriptedChatModel has only 2 responses; after that it errors.
    // With caching, repeated requests are served from cache.
    let model = ScriptedChatModel::new(vec![
        ChatResponse {
            message: Message::ai("The capital of France is Paris."),
            usage: None,
        },
        ChatResponse {
            message: Message::ai("Rust was created by Graydon Hoare."),
            usage: None,
        },
    ]);

    let cache = Arc::new(InMemoryCache::new());
    let cached_model = CachedChatModel::new(Arc::new(model), cache.clone());

    // --- First call: cache miss ---
    println!("=== Cache Miss (first call) ===");
    let request1 = ChatRequest::new(vec![Message::human("What is the capital of France?")]);
    let response1 = cached_model.chat(request1.clone()).await?;
    println!("Response: {}", response1.message.content());

    // --- Second call with same request: cache hit ---
    println!("\n=== Cache Hit (same request) ===");
    let response2 = cached_model.chat(request1.clone()).await?;
    println!("Response: {}", response2.message.content());
    println!(
        "Same response: {}",
        response1.message.content() == response2.message.content()
    );

    // --- Third call with different request: cache miss ---
    println!("\n=== Cache Miss (different request) ===");
    let request2 = ChatRequest::new(vec![Message::human("Who created Rust?")]);
    let response3 = cached_model.chat(request2.clone()).await?;
    println!("Response: {}", response3.message.content());

    // --- Fourth call: cache hit for second request ---
    println!("\n=== Cache Hit (second request again) ===");
    let response4 = cached_model.chat(request2).await?;
    println!("Response: {}", response4.message.content());
    println!(
        "Same response: {}",
        response3.message.content() == response4.message.content()
    );

    // --- Clear cache ---
    println!("\n=== Clear Cache ===");
    cache.clear().await?;
    println!("Cache cleared");

    println!("\nCaching demo completed successfully!");
    Ok(())
}
