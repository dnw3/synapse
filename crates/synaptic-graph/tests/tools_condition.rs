use synaptic_core::{Message, ToolCall};
use synaptic_graph::{tools_condition, MessageState, END};

#[test]
fn tools_condition_returns_tools_when_tool_calls_present() {
    let state = MessageState::with_messages(vec![Message::ai_with_tool_calls(
        "",
        vec![ToolCall {
            id: "call-1".to_string(),
            name: "search".to_string(),
            arguments: serde_json::json!({"q": "test"}),
        }],
    )]);

    assert_eq!(tools_condition(&state), "tools");
}

#[test]
fn tools_condition_returns_end_when_no_tool_calls() {
    let state = MessageState::with_messages(vec![Message::ai("Just a text response")]);

    assert_eq!(tools_condition(&state), END);
}

#[test]
fn tools_condition_returns_end_for_empty_state() {
    let state = MessageState::with_messages(vec![]);

    assert_eq!(tools_condition(&state), END);
}

#[test]
fn tools_condition_checks_last_message_only() {
    // First message has tool calls, but last message does not
    let state = MessageState::with_messages(vec![
        Message::ai_with_tool_calls(
            "",
            vec![ToolCall {
                id: "call-1".to_string(),
                name: "search".to_string(),
                arguments: serde_json::json!({}),
            }],
        ),
        Message::tool("result", "call-1"),
        Message::ai("Final answer"),
    ]);

    // Should return END because the *last* message has no tool calls
    assert_eq!(tools_condition(&state), END);
}

#[test]
fn tools_condition_returns_tools_with_multiple_tool_calls() {
    let state = MessageState::with_messages(vec![Message::ai_with_tool_calls(
        "",
        vec![
            ToolCall {
                id: "call-1".to_string(),
                name: "search".to_string(),
                arguments: serde_json::json!({}),
            },
            ToolCall {
                id: "call-2".to_string(),
                name: "calculator".to_string(),
                arguments: serde_json::json!({}),
            },
        ],
    )]);

    assert_eq!(tools_condition(&state), "tools");
}
