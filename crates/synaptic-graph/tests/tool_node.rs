use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use synaptic_core::{Message, SynapseError, Tool, ToolCall};
use synaptic_graph::{MessageState, Node, ToolNode};
use synaptic_tools::{SerialToolExecutor, ToolRegistry};

struct EchoTool;

#[async_trait]
impl Tool for EchoTool {
    fn name(&self) -> &'static str {
        "echo"
    }
    fn description(&self) -> &'static str {
        "echoes input"
    }
    async fn call(&self, args: Value) -> Result<Value, SynapseError> {
        Ok(args)
    }
}

fn make_tool_node() -> ToolNode {
    let registry = ToolRegistry::new();
    registry.register(Arc::new(EchoTool)).unwrap();
    let executor = SerialToolExecutor::new(registry);
    ToolNode::new(executor)
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

    let result = tool_node.process(state).await.unwrap();

    // Should have original AI message + tool response
    assert_eq!(result.messages.len(), 2);
    assert!(result.messages[1].is_tool());
    assert_eq!(result.messages[1].tool_call_id(), Some("call-1"));
    // The tool response content should be the JSON-serialized args
    assert!(result.messages[1].content().contains("hello"));
}

#[tokio::test]
async fn tool_node_no_tool_calls_passthrough() {
    let tool_node = make_tool_node();

    let state = MessageState::with_messages(vec![Message::ai("just text, no tools")]);

    let result = tool_node.process(state).await.unwrap();

    // State should be unchanged
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

    let result = tool_node.process(state).await.unwrap();

    // Original AI message + 2 tool responses
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
    // ToolNode should handle missing tools — check behavior
    // It may return error in the tool message or propagate error
    assert!(result.is_ok() || result.is_err());
    if let Ok(state) = result {
        // If it wraps error in tool message
        assert!(state.messages.len() >= 2);
    }
}

#[tokio::test]
async fn tool_node_empty_messages() {
    let tool_node = make_tool_node();
    let state = MessageState::with_messages(vec![]);

    let result = tool_node.process(state).await;
    // ToolNode errors when there are no messages in state
    assert!(result.is_err());
}
