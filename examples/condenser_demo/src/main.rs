use std::sync::Arc;
use synaptic::condenser::{Condenser, PipelineCondenser, RollingCondenser, TokenBudgetCondenser};
use synaptic::core::{HeuristicTokenCounter, Message};

#[tokio::main]
async fn main() {
    // Build a conversation with many messages
    let messages = vec![
        Message::system("You are a helpful assistant."),
        Message::human("What is Rust?"),
        Message::ai("Rust is a systems programming language focused on safety and performance."),
        Message::human("How does ownership work?"),
        Message::ai("Ownership is Rust's memory management model with three rules: each value has an owner, only one owner at a time, and the value is dropped when the owner goes out of scope."),
        Message::human("What about borrowing?"),
        Message::ai("Borrowing lets you reference data without taking ownership. You can have many immutable references or one mutable reference at a time."),
        Message::human("Can you explain lifetimes?"),
        Message::ai("Lifetimes are annotations that tell the compiler how long references are valid, preventing dangling references."),
        Message::human("What are traits?"),
        Message::ai("Traits define shared behavior, similar to interfaces in other languages."),
    ];

    println!("=== Condenser Demo ===\n");
    println!("Original message count: {}\n", messages.len());

    // 1. RollingCondenser: keep last 5 messages, preserve system
    let rolling = RollingCondenser::new(5);
    let condensed = rolling.condense(messages.clone()).await.unwrap();
    println!("--- RollingCondenser (max 5, preserve system) ---");
    println!("After condensing: {} messages", condensed.len());
    for msg in &condensed {
        println!(
            "  [{}] {}...",
            msg.role(),
            &msg.content()[..msg.content().len().min(60)]
        );
    }

    // 2. TokenBudgetCondenser: fit within 100 tokens
    let counter = Arc::new(HeuristicTokenCounter);
    let token_budget = TokenBudgetCondenser::new(100, counter.clone());
    let condensed = token_budget.condense(messages.clone()).await.unwrap();
    println!("\n--- TokenBudgetCondenser (100 tokens) ---");
    println!("After condensing: {} messages", condensed.len());
    for msg in &condensed {
        println!(
            "  [{}] {}...",
            msg.role(),
            &msg.content()[..msg.content().len().min(60)]
        );
    }

    // 3. PipelineCondenser: rolling then token budget
    let pipeline = PipelineCondenser::new(vec![
        Arc::new(RollingCondenser::new(7)),
        Arc::new(TokenBudgetCondenser::new(150, counter)),
    ]);
    let condensed = pipeline.condense(messages.clone()).await.unwrap();
    println!("\n--- PipelineCondenser (rolling 7 -> token budget 150) ---");
    println!("After condensing: {} messages", condensed.len());
    for msg in &condensed {
        println!(
            "  [{}] {}...",
            msg.role(),
            &msg.content()[..msg.content().len().min(60)]
        );
    }

    println!("\nDone.");
}
