use serde_json::json;
use synapse_core::{AIMessageChunk, Message, TokenUsage, ToolCall};

#[test]
fn system_message_factory() {
    let msg = Message::system("You are helpful");
    assert_eq!(msg.content(), "You are helpful");
    assert_eq!(msg.role(), "system");
    assert!(msg.is_system());
    assert!(!msg.is_human());
}

#[test]
fn human_message_factory() {
    let msg = Message::human("Hello");
    assert_eq!(msg.content(), "Hello");
    assert_eq!(msg.role(), "human");
    assert!(msg.is_human());
}

#[test]
fn ai_message_factory() {
    let msg = Message::ai("I can help");
    assert_eq!(msg.content(), "I can help");
    assert_eq!(msg.role(), "assistant");
    assert!(msg.is_ai());
    assert!(msg.tool_calls().is_empty());
}

#[test]
fn ai_message_with_tool_calls() {
    let msg = Message::ai_with_tool_calls(
        "calling tool",
        vec![ToolCall {
            id: "call-1".into(),
            name: "search".into(),
            arguments: json!({"q": "rust"}),
        }],
    );
    assert_eq!(msg.tool_calls().len(), 1);
    assert_eq!(msg.tool_calls()[0].name, "search");
}

#[test]
fn tool_message_factory() {
    let msg = Message::tool("result data", "call-1");
    assert_eq!(msg.content(), "result data");
    assert_eq!(msg.role(), "tool");
    assert!(msg.is_tool());
    assert_eq!(msg.tool_call_id(), Some("call-1"));
}

#[test]
fn tool_call_id_none_for_non_tool() {
    let msg = Message::human("hi");
    assert_eq!(msg.tool_call_id(), None);
}

#[test]
fn message_serde_roundtrip() {
    let msg = Message::ai_with_tool_calls(
        "using tool",
        vec![ToolCall {
            id: "c1".into(),
            name: "calc".into(),
            arguments: json!({"x": 1}),
        }],
    );
    let json = serde_json::to_string(&msg).unwrap();
    let deserialized: Message = serde_json::from_str(&json).unwrap();
    assert_eq!(msg, deserialized);
}

#[test]
fn message_serde_system_format() {
    let msg = Message::system("be helpful");
    let json = serde_json::to_value(&msg).unwrap();
    assert_eq!(json["role"], "system");
    assert_eq!(json["content"], "be helpful");
}

#[test]
fn message_serde_tool_calls_omitted_when_empty() {
    let msg = Message::ai("hello");
    let json = serde_json::to_value(&msg).unwrap();
    assert!(json.get("tool_calls").is_none());
}

#[test]
fn chunk_add_concatenates_content() {
    let a = AIMessageChunk {
        content: "Hello".into(),
        tool_calls: vec![],
        usage: None,
    };
    let b = AIMessageChunk {
        content: " world".into(),
        tool_calls: vec![],
        usage: None,
    };
    let merged = a + b;
    assert_eq!(merged.content, "Hello world");
}

#[test]
fn chunk_add_merges_tool_calls() {
    let a = AIMessageChunk {
        content: String::new(),
        tool_calls: vec![ToolCall {
            id: "c1".into(),
            name: "search".into(),
            arguments: json!({}),
        }],
        usage: None,
    };
    let b = AIMessageChunk {
        content: String::new(),
        tool_calls: vec![ToolCall {
            id: "c2".into(),
            name: "calc".into(),
            arguments: json!({}),
        }],
        usage: None,
    };
    let merged = a + b;
    assert_eq!(merged.tool_calls.len(), 2);
}

#[test]
fn chunk_add_merges_usage() {
    let a = AIMessageChunk {
        content: "a".into(),
        tool_calls: vec![],
        usage: Some(TokenUsage {
            input_tokens: 10,
            output_tokens: 5,
            total_tokens: 15,
        }),
    };
    let b = AIMessageChunk {
        content: "b".into(),
        tool_calls: vec![],
        usage: Some(TokenUsage {
            input_tokens: 0,
            output_tokens: 3,
            total_tokens: 3,
        }),
    };
    let merged = a + b;
    let usage = merged.usage.unwrap();
    assert_eq!(usage.input_tokens, 10);
    assert_eq!(usage.output_tokens, 8);
    assert_eq!(usage.total_tokens, 18);
}

#[test]
fn chunk_add_assign_works() {
    let mut chunk = AIMessageChunk {
        content: "Hello".into(),
        tool_calls: vec![],
        usage: None,
    };
    chunk += AIMessageChunk {
        content: " world".into(),
        tool_calls: vec![],
        usage: None,
    };
    assert_eq!(chunk.content, "Hello world");
}

#[test]
fn chunk_into_message() {
    let chunk = AIMessageChunk {
        content: "final answer".into(),
        tool_calls: vec![ToolCall {
            id: "c1".into(),
            name: "tool".into(),
            arguments: json!({}),
        }],
        usage: None,
    };
    let msg = chunk.into_message();
    assert!(msg.is_ai());
    assert_eq!(msg.content(), "final answer");
    assert_eq!(msg.tool_calls().len(), 1);
}
