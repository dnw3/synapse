use std::sync::Arc;
use synaptic_core::{
    ContextBudget, ContextSlot, HeuristicTokenCounter, Message, Priority, TokenCounter,
};

#[test]
fn heuristic_basic() {
    let counter = HeuristicTokenCounter;
    assert_eq!(counter.count_text(""), 0);
    // 12 chars / 4 = 3 tokens
    assert_eq!(counter.count_text("hello world!"), 3);
    // Short text: at least 1
    assert_eq!(counter.count_text("hi"), 1);
}

#[test]
fn count_messages_overhead() {
    let counter = HeuristicTokenCounter;
    let messages = vec![
        Message::human("hello"), // 1 token + 4 overhead = 5
        Message::ai("world"),    // 1 token + 4 overhead = 5
    ];
    assert_eq!(counter.count_messages(&messages), 10);
}

#[test]
fn budget_within_limit() {
    let counter = Arc::new(HeuristicTokenCounter);
    let budget = ContextBudget::new(1000, counter);

    let slots = vec![
        ContextSlot {
            name: "system".to_string(),
            priority: Priority::CRITICAL,
            messages: vec![Message::system("You are helpful")],
            reserved_tokens: 0,
        },
        ContextSlot {
            name: "history".to_string(),
            priority: Priority::NORMAL,
            messages: vec![Message::human("Hi"), Message::ai("Hello!")],
            reserved_tokens: 0,
        },
    ];

    let result = budget.assemble(slots);
    assert_eq!(result.len(), 3);
}

#[test]
fn respects_priority() {
    let counter = Arc::new(HeuristicTokenCounter);
    // Very tight budget
    let budget = ContextBudget::new(10, counter);

    let slots = vec![
        ContextSlot {
            name: "low".to_string(),
            priority: Priority::LOW,
            messages: vec![Message::human("low priority message here")],
            reserved_tokens: 0,
        },
        ContextSlot {
            name: "critical".to_string(),
            priority: Priority::CRITICAL,
            messages: vec![Message::system("hi")],
            reserved_tokens: 0,
        },
    ];

    let result = budget.assemble(slots);
    // Critical should be first (sorted by priority)
    assert!(result[0].is_system());
}

#[test]
fn drops_low_priority() {
    let counter = Arc::new(HeuristicTokenCounter);
    // Budget only fits ~1-2 messages
    let budget = ContextBudget::new(12, counter);

    let slots = vec![
        ContextSlot {
            name: "system".to_string(),
            priority: Priority::CRITICAL,
            messages: vec![Message::system("You are helpful")],
            reserved_tokens: 0,
        },
        ContextSlot {
            name: "extra".to_string(),
            priority: Priority::LOW,
            messages: vec![Message::human(
                "a very long message that should exceed budget limits easily",
            )],
            reserved_tokens: 0,
        },
    ];

    let result = budget.assemble(slots);
    // Only the critical message should fit
    assert_eq!(result.len(), 1);
    assert!(result[0].is_system());
}

#[test]
fn reserved_honored() {
    let counter = Arc::new(HeuristicTokenCounter);
    let budget = ContextBudget::new(20, counter);

    let slots = vec![
        ContextSlot {
            name: "reserved".to_string(),
            priority: Priority::HIGH,
            messages: vec![Message::system("reserved")],
            reserved_tokens: 10,
        },
        ContextSlot {
            name: "normal".to_string(),
            priority: Priority::NORMAL,
            messages: vec![Message::human("hi")],
            reserved_tokens: 0,
        },
    ];

    let result = budget.assemble(slots);
    // Reserved slot should be included
    assert!(result.iter().any(|m| m.is_system()));
}
