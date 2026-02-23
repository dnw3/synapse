use std::sync::Arc;

use synaptic::core::{
    ContextBudget, ContextSlot, HeuristicTokenCounter, Message, Priority, TokenCounter,
};

#[tokio::main]
async fn main() {
    println!("=== ContextBudget Priority Demo ===\n");

    let counter = Arc::new(HeuristicTokenCounter);

    // Create slots with different priorities
    let system_slot = ContextSlot {
        name: "system".to_string(),
        priority: Priority::CRITICAL,
        messages: vec![Message::system("You are a helpful assistant.")],
        reserved_tokens: 20,
    };

    let recent_slot = ContextSlot {
        name: "recent_messages".to_string(),
        priority: Priority::HIGH,
        messages: vec![
            Message::human("What is Rust?"),
            Message::ai("Rust is a systems programming language."),
        ],
        reserved_tokens: 0,
    };

    let tool_results_slot = ContextSlot {
        name: "tool_results".to_string(),
        priority: Priority::NORMAL,
        messages: vec![Message::tool(
            "File contents: fn main() { println!(\"Hello\"); }",
            "call-1",
        )],
        reserved_tokens: 0,
    };

    let history_slot = ContextSlot {
        name: "old_history".to_string(),
        priority: Priority::LOW,
        messages: vec![
            Message::human("Earlier question about Python"),
            Message::ai("Python is an interpreted language known for simplicity."),
            Message::human("What about JavaScript?"),
            Message::ai("JavaScript is the language of the web, running in browsers and Node.js."),
        ],
        reserved_tokens: 0,
    };

    // Show token counts per slot
    println!("--- Slot token counts (heuristic: ~4 chars/token + 4 overhead/msg) ---");
    for (name, msgs) in [
        ("system", &system_slot.messages),
        ("recent_messages", &recent_slot.messages),
        ("tool_results", &tool_results_slot.messages),
        ("old_history", &history_slot.messages),
    ] {
        let tokens = counter.count_messages(msgs);
        println!("  {:<20} {} tokens ({} messages)", name, tokens, msgs.len());
    }

    // Assemble with a generous budget: everything fits
    println!("\n--- Budget: 500 tokens (generous) ---");
    let budget = ContextBudget::new(500, counter.clone());
    let result = budget.assemble(vec![
        make_slot(
            "system",
            Priority::CRITICAL,
            &[Message::system("You are a helpful assistant.")],
            20,
        ),
        make_slot(
            "recent_messages",
            Priority::HIGH,
            &[
                Message::human("What is Rust?"),
                Message::ai("Rust is a systems programming language."),
            ],
            0,
        ),
        make_slot(
            "tool_results",
            Priority::NORMAL,
            &[Message::tool(
                "File contents: fn main() { println!(\"Hello\"); }",
                "call-1",
            )],
            0,
        ),
        make_slot(
            "old_history",
            Priority::LOW,
            &[
                Message::human("Earlier question about Python"),
                Message::ai("Python is an interpreted language known for simplicity."),
                Message::human("What about JavaScript?"),
                Message::ai(
                    "JavaScript is the language of the web, running in browsers and Node.js.",
                ),
            ],
            0,
        ),
    ]);
    println!("  Assembled {} messages", result.len());
    for msg in &result {
        println!(
            "    [{}] {}...",
            msg.role(),
            &msg.content()[..msg.content().len().min(50)]
        );
    }

    // Assemble with a tight budget: low-priority slots get dropped
    println!("\n--- Budget: 80 tokens (tight) ---");
    let budget = ContextBudget::new(80, counter.clone());
    let result = budget.assemble(vec![
        make_slot(
            "system",
            Priority::CRITICAL,
            &[Message::system("You are a helpful assistant.")],
            20,
        ),
        make_slot(
            "recent_messages",
            Priority::HIGH,
            &[
                Message::human("What is Rust?"),
                Message::ai("Rust is a systems programming language."),
            ],
            0,
        ),
        make_slot(
            "tool_results",
            Priority::NORMAL,
            &[Message::tool(
                "File contents: fn main() { println!(\"Hello\"); }",
                "call-1",
            )],
            0,
        ),
        make_slot(
            "old_history",
            Priority::LOW,
            &[
                Message::human("Earlier question about Python"),
                Message::ai("Python is an interpreted language known for simplicity."),
                Message::human("What about JavaScript?"),
                Message::ai(
                    "JavaScript is the language of the web, running in browsers and Node.js.",
                ),
            ],
            0,
        ),
    ]);
    println!(
        "  Assembled {} messages (lower-priority slots dropped)",
        result.len()
    );
    for msg in &result {
        println!(
            "    [{}] {}...",
            msg.role(),
            &msg.content()[..msg.content().len().min(50)]
        );
    }

    println!("\nDone.");
}

fn make_slot(name: &str, priority: Priority, messages: &[Message], reserved: usize) -> ContextSlot {
    ContextSlot {
        name: name.to_string(),
        priority,
        messages: messages.to_vec(),
        reserved_tokens: reserved,
    }
}
