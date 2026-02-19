use serde_json::json;
use synaptic_core::{Message, ToolCall};
use synaptic_middleware::{AgentMiddleware, ContextEditingMiddleware, ModelRequest};

fn make_request(messages: Vec<Message>) -> ModelRequest {
    ModelRequest {
        messages,
        tools: vec![],
        tool_choice: None,
        system_prompt: None,
    }
}

// ---------------------------------------------------------------------------
// LastN tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn last_n_keeps_only_n_messages() {
    let mw = ContextEditingMiddleware::last_n(2);
    let mut req = make_request(vec![
        Message::human("1"),
        Message::ai("2"),
        Message::human("3"),
        Message::ai("4"),
        Message::human("5"),
    ]);

    mw.before_model(&mut req).await.unwrap();
    assert_eq!(req.messages.len(), 2);
    assert_eq!(req.messages[0].content(), "4");
    assert_eq!(req.messages[1].content(), "5");
}

#[tokio::test]
async fn last_n_preserves_leading_system_messages() {
    let mw = ContextEditingMiddleware::last_n(2);
    let mut req = make_request(vec![
        Message::system("You are helpful."),
        Message::human("1"),
        Message::ai("2"),
        Message::human("3"),
        Message::ai("4"),
    ]);

    mw.before_model(&mut req).await.unwrap();
    // System message + last 2 non-system messages
    assert_eq!(req.messages.len(), 3);
    assert!(req.messages[0].is_system());
    assert_eq!(req.messages[1].content(), "3");
    assert_eq!(req.messages[2].content(), "4");
}

#[tokio::test]
async fn last_n_leaves_shorter_list_unchanged() {
    let mw = ContextEditingMiddleware::last_n(10);
    let original_messages = vec![Message::human("a"), Message::ai("b")];
    let mut req = make_request(original_messages.clone());

    mw.before_model(&mut req).await.unwrap();
    assert_eq!(req.messages.len(), 2);
    assert_eq!(req.messages[0].content(), "a");
    assert_eq!(req.messages[1].content(), "b");
}

// ---------------------------------------------------------------------------
// StripToolCalls tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn strip_tool_calls_removes_tool_messages() {
    let mw = ContextEditingMiddleware::strip_tool_calls();
    let mut req = make_request(vec![
        Message::human("hello"),
        Message::ai_with_tool_calls(
            "",
            vec![ToolCall {
                id: "tc-1".into(),
                name: "search".into(),
                arguments: json!({}),
            }],
        ),
        Message::tool("search result", "tc-1"),
        Message::ai("final answer"),
    ]);

    mw.before_model(&mut req).await.unwrap();

    // Only human and the final AI message should remain
    assert_eq!(req.messages.len(), 2);
    assert!(req.messages[0].is_human());
    assert_eq!(req.messages[0].content(), "hello");
    assert!(req.messages[1].is_ai());
    assert_eq!(req.messages[1].content(), "final answer");
}

#[tokio::test]
async fn strip_tool_calls_preserves_ai_with_content() {
    let mw = ContextEditingMiddleware::strip_tool_calls();

    // An AI message that has both content and tool calls should be kept
    // only if it has non-empty content. The implementation strips AI messages
    // with tool calls AND empty content.
    let mut req = make_request(vec![
        Message::human("hello"),
        Message::ai_with_tool_calls(
            "Let me search for that.",
            vec![ToolCall {
                id: "tc-1".into(),
                name: "search".into(),
                arguments: json!({}),
            }],
        ),
        Message::tool("search result", "tc-1"),
        Message::ai("done"),
    ]);

    mw.before_model(&mut req).await.unwrap();

    // Human + AI-with-content (not stripped because it has text) + AI("done")
    // Tool message IS stripped
    assert_eq!(req.messages.len(), 3);
    assert!(req.messages[0].is_human());
    assert!(req.messages[1].is_ai());
    assert_eq!(req.messages[1].content(), "Let me search for that.");
    assert_eq!(req.messages[2].content(), "done");
}
