use synaptic_core::{
    filter_messages, get_buffer_string, merge_message_runs, trim_messages, Message, ToolCall,
    TrimStrategy,
};

// ---------------------------------------------------------------------------
// filter_messages
// ---------------------------------------------------------------------------

#[test]
fn filter_messages_include_types() {
    let msgs = vec![
        Message::system("sys"),
        Message::human("hi"),
        Message::ai("hello"),
        Message::tool("result", "c1"),
    ];
    let filtered = filter_messages(&msgs, Some(&["human", "assistant"]), None, None, None, None, None);
    assert_eq!(filtered.len(), 2);
    assert!(filtered[0].is_human());
    assert!(filtered[1].is_ai());
}

#[test]
fn filter_messages_exclude_types() {
    let msgs = vec![
        Message::system("sys"),
        Message::human("hi"),
        Message::ai("hello"),
    ];
    let filtered = filter_messages(&msgs, None, Some(&["system"]), None, None, None, None);
    assert_eq!(filtered.len(), 2);
    assert!(filtered[0].is_human());
    assert!(filtered[1].is_ai());
}

#[test]
fn filter_messages_include_names() {
    let msgs = vec![
        Message::human("hi").with_name("alice"),
        Message::human("hey").with_name("bob"),
        Message::human("yo"), // no name → excluded
    ];
    let filtered = filter_messages(&msgs, None, None, Some(&["alice"]), None, None, None);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].name(), Some("alice"));
}

#[test]
fn filter_messages_exclude_names() {
    let msgs = vec![
        Message::human("hi").with_name("alice"),
        Message::human("hey").with_name("bob"),
        Message::human("yo"), // no name → kept (exclude only matches named)
    ];
    let filtered = filter_messages(&msgs, None, None, None, Some(&["alice"]), None, None);
    assert_eq!(filtered.len(), 2);
    assert_eq!(filtered[0].name(), Some("bob"));
    assert!(filtered[1].name().is_none());
}

#[test]
fn filter_messages_include_ids() {
    let msgs = vec![
        Message::human("hi").with_id("msg-1"),
        Message::human("hey").with_id("msg-2"),
        Message::human("yo"), // no id → excluded
    ];
    let filtered = filter_messages(&msgs, None, None, None, None, Some(&["msg-2"]), None);
    assert_eq!(filtered.len(), 1);
    assert_eq!(filtered[0].id(), Some("msg-2"));
}

#[test]
fn filter_messages_exclude_ids() {
    let msgs = vec![
        Message::human("hi").with_id("msg-1"),
        Message::human("hey").with_id("msg-2"),
        Message::human("yo"),
    ];
    let filtered = filter_messages(&msgs, None, None, None, None, None, Some(&["msg-1"]));
    assert_eq!(filtered.len(), 2);
    assert_eq!(filtered[0].id(), Some("msg-2"));
    assert!(filtered[1].id().is_none());
}

#[test]
fn filter_messages_empty_input() {
    let filtered = filter_messages(&[], Some(&["human"]), None, None, None, None, None);
    assert!(filtered.is_empty());
}

#[test]
fn filter_messages_no_filters() {
    let msgs = vec![Message::human("hi"), Message::ai("hello")];
    let filtered = filter_messages(&msgs, None, None, None, None, None, None);
    assert_eq!(filtered.len(), 2);
}

// ---------------------------------------------------------------------------
// trim_messages
// ---------------------------------------------------------------------------

fn char_counter(msg: &Message) -> usize {
    msg.content().len()
}

#[test]
fn trim_messages_first_strategy() {
    let msgs = vec![
        Message::human("hello"),     // 5
        Message::ai("world"),        // 5
        Message::human("goodbye"),   // 7
    ];
    let trimmed = trim_messages(msgs, 10, char_counter, TrimStrategy::First, false);
    assert_eq!(trimmed.len(), 2);
    assert_eq!(trimmed[0].content(), "hello");
    assert_eq!(trimmed[1].content(), "world");
}

#[test]
fn trim_messages_last_strategy() {
    let msgs = vec![
        Message::human("hello"),     // 5
        Message::ai("world"),        // 5
        Message::human("goodbye"),   // 7
    ];
    let trimmed = trim_messages(msgs, 12, char_counter, TrimStrategy::Last, false);
    assert_eq!(trimmed.len(), 2);
    assert_eq!(trimmed[0].content(), "world");
    assert_eq!(trimmed[1].content(), "goodbye");
}

#[test]
fn trim_messages_last_with_system() {
    let msgs = vec![
        Message::system("sys"),      // 3
        Message::human("hello"),     // 5
        Message::ai("world"),        // 5
        Message::human("goodbye"),   // 7
    ];
    // Budget: 15, system takes 3, leaving 12 for rest → world(5) + goodbye(7) = 12
    let trimmed = trim_messages(msgs, 15, char_counter, TrimStrategy::Last, true);
    assert_eq!(trimmed.len(), 3);
    assert!(trimmed[0].is_system());
    assert_eq!(trimmed[1].content(), "world");
    assert_eq!(trimmed[2].content(), "goodbye");
}

#[test]
fn trim_messages_exact_budget() {
    let msgs = vec![
        Message::human("hi"),  // 2
        Message::ai("ok"),     // 2
    ];
    let trimmed = trim_messages(msgs.clone(), 4, char_counter, TrimStrategy::First, false);
    assert_eq!(trimmed.len(), 2);
}

#[test]
fn trim_messages_empty_input() {
    let trimmed = trim_messages(vec![], 100, char_counter, TrimStrategy::First, false);
    assert!(trimmed.is_empty());
}

#[test]
fn trim_messages_zero_budget() {
    let msgs = vec![Message::human("hello")];
    let trimmed = trim_messages(msgs, 0, char_counter, TrimStrategy::First, false);
    assert!(trimmed.is_empty());
}

// ---------------------------------------------------------------------------
// merge_message_runs
// ---------------------------------------------------------------------------

#[test]
fn merge_message_runs_consecutive_human() {
    let msgs = vec![
        Message::human("hello"),
        Message::human("world"),
    ];
    let merged = merge_message_runs(msgs);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].content(), "hello\nworld");
    assert!(merged[0].is_human());
}

#[test]
fn merge_message_runs_consecutive_ai() {
    let msgs = vec![
        Message::ai("part 1"),
        Message::ai("part 2"),
    ];
    let merged = merge_message_runs(msgs);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].content(), "part 1\npart 2");
    assert!(merged[0].is_ai());
}

#[test]
fn merge_message_runs_ai_with_tool_calls() {
    let msgs = vec![
        Message::ai_with_tool_calls(
            "calling",
            vec![ToolCall {
                id: "c1".into(),
                name: "search".into(),
                arguments: serde_json::json!({}),
            }],
        ),
        Message::ai_with_tool_calls(
            "more",
            vec![ToolCall {
                id: "c2".into(),
                name: "calc".into(),
                arguments: serde_json::json!({}),
            }],
        ),
    ];
    let merged = merge_message_runs(msgs);
    assert_eq!(merged.len(), 1);
    assert_eq!(merged[0].tool_calls().len(), 2);
    assert_eq!(merged[0].content(), "calling\nmore");
}

#[test]
fn merge_message_runs_alternating() {
    let msgs = vec![
        Message::human("hi"),
        Message::ai("hello"),
        Message::human("bye"),
    ];
    let merged = merge_message_runs(msgs);
    assert_eq!(merged.len(), 3);
}

#[test]
fn merge_message_runs_empty() {
    let merged = merge_message_runs(vec![]);
    assert!(merged.is_empty());
}

#[test]
fn merge_message_runs_remove_not_merged() {
    let msgs = vec![
        Message::remove("msg-1"),
        Message::remove("msg-2"),
    ];
    // Remove messages have role "remove" and are equal, but the impl
    // doesn't mutate Remove content — they should still merge by role matching.
    // Let's just verify no panic occurs.
    let merged = merge_message_runs(msgs);
    // Both have role "remove" so they get merged
    assert_eq!(merged.len(), 1);
}

// ---------------------------------------------------------------------------
// get_buffer_string
// ---------------------------------------------------------------------------

#[test]
fn get_buffer_string_default_prefixes() {
    let msgs = vec![
        Message::system("You are helpful"),
        Message::human("Hello"),
        Message::ai("Hi there"),
    ];
    let buffer = get_buffer_string(&msgs, "Human", "AI");
    assert_eq!(buffer, "System: You are helpful\nHuman: Hello\nAI: Hi there");
}

#[test]
fn get_buffer_string_custom_prefixes() {
    let msgs = vec![
        Message::human("Hello"),
        Message::ai("Hi"),
    ];
    let buffer = get_buffer_string(&msgs, "User", "Assistant");
    assert_eq!(buffer, "User: Hello\nAssistant: Hi");
}

#[test]
fn get_buffer_string_empty() {
    let buffer = get_buffer_string(&[], "Human", "AI");
    assert_eq!(buffer, "");
}

#[test]
fn get_buffer_string_tool_messages() {
    let msgs = vec![
        Message::tool("result", "c1"),
    ];
    let buffer = get_buffer_string(&msgs, "Human", "AI");
    assert_eq!(buffer, "Tool: result");
}
