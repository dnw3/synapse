use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use synapse_core::{Message, SynapseError, Tool, ToolCall};
use synapse_graph::{MessageState, Node, ToolNode};
use synapse_tools::{SerialToolExecutor, ToolRegistry};

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
