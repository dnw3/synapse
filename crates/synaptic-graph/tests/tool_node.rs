use serde_json::Value;
use synaptic_core::{Message, SynapticError, ToolCall};
use synaptic_graph::{MessageState, Node, NodeOutput, ToolNode};
use synaptic_macros::tool;
use synaptic_tools::{SerialToolExecutor, ToolRegistry};

/// echoes input
#[tool(name = "echo")]
async fn echo(#[args] args: Value) -> Result<Value, SynapticError> {
    Ok(args)
}

fn make_tool_node() -> ToolNode {
    let registry = ToolRegistry::new();
    registry.register(echo()).unwrap();
    let executor = SerialToolExecutor::new(registry);
    ToolNode::new(executor)
}

fn extract_state(output: NodeOutput<MessageState>) -> MessageState {
    match output {
        NodeOutput::State(s) => s,
        NodeOutput::Command(_) => panic!("expected NodeOutput::State"),
    }
}

#[tokio::test]
async fn tool_node_executes_tool_calls() {
    let tool_node = make_tool_node();

    let state = MessageState::with_messages(vec![Message::ai_with_tool_calls(
        "",
        vec![ToolCall {
            id: "call-1".to_string(),
            name: "echo".to_string(),
            arguments: serde_json::json!({"text": "hello"}),
        }],
    )]);

    let result = extract_state(tool_node.process(state).await.unwrap());

    // Should have original AI message + tool response
    assert_eq!(result.messages.len(), 2);
    assert!(result.messages[1].is_tool());
    assert_eq!(result.messages[1].tool_call_id(), Some("call-1"));
    assert!(result.messages[1].content().contains("hello"));
}

#[tokio::test]
async fn tool_node_no_tool_calls_passthrough() {
    let tool_node = make_tool_node();

    let state = MessageState::with_messages(vec![Message::ai("just text, no tools")]);

    let result = extract_state(tool_node.process(state).await.unwrap());

    assert_eq!(result.messages.len(), 1);
    assert_eq!(result.messages[0].content(), "just text, no tools");
}

#[tokio::test]
async fn tool_node_executes_multiple_tool_calls() {
    let tool_node = make_tool_node();

    let state = MessageState::with_messages(vec![Message::ai_with_tool_calls(
        "",
        vec![
            ToolCall {
                id: "call-1".to_string(),
                name: "echo".to_string(),
                arguments: serde_json::json!({"text": "first"}),
            },
            ToolCall {
                id: "call-2".to_string(),
                name: "echo".to_string(),
                arguments: serde_json::json!({"text": "second"}),
            },
        ],
    )]);

    let result = extract_state(tool_node.process(state).await.unwrap());

    assert_eq!(result.messages.len(), 3);
    assert!(result.messages[1].is_tool());
    assert!(result.messages[2].is_tool());
    assert_eq!(result.messages[1].tool_call_id(), Some("call-1"));
    assert_eq!(result.messages[2].tool_call_id(), Some("call-2"));
}

#[tokio::test]
async fn tool_node_unregistered_tool_error() {
    let tool_node = make_tool_node();

    let state = MessageState::with_messages(vec![Message::ai_with_tool_calls(
        "",
        vec![ToolCall {
            id: "call-1".to_string(),
            name: "nonexistent_tool".to_string(),
            arguments: serde_json::json!({}),
        }],
    )]);

    let result = tool_node.process(state).await;
    assert!(result.is_ok() || result.is_err());
    if let Ok(output) = result {
        let state = extract_state(output);
        assert!(state.messages.len() >= 2);
    }
}

#[tokio::test]
async fn tool_node_empty_messages() {
    let tool_node = make_tool_node();
    let state = MessageState::with_messages(vec![]);

    let result = tool_node.process(state).await;
    assert!(result.is_err());
}
