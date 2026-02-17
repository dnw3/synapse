use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use synaptic_core::{ChatResponse, Message, SynapseError, Tool, ToolCall};
use synaptic_graph::{create_react_agent, MessageState};
use synaptic_models::ScriptedChatModel;

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

#[test]
fn create_react_agent_compiles() {
    let model = Arc::new(ScriptedChatModel::new(vec![]));
    let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(EchoTool)];
    let result = create_react_agent(model, tools);
    assert!(result.is_ok());
}

#[tokio::test]
async fn react_agent_no_tool_calls() {
    // Model returns a plain text response (no tool calls) => agent should complete
    let model = Arc::new(ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai("Hello, how can I help?"),
        usage: None,
    }]));

    let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(EchoTool)];
    let graph = create_react_agent(model, tools).unwrap();

    let state = MessageState::with_messages(vec![Message::human("hi")]);
    let result = graph.invoke(state).await.unwrap();

    // Should have: human message + AI response
    assert_eq!(result.messages.len(), 2);
    assert_eq!(result.messages[1].content(), "Hello, how can I help?");
    assert!(result.messages[1].is_ai());
}

#[tokio::test]
async fn react_agent_with_tool_calls() {
    // First response: AI with tool call
    // Second response: AI with plain text (after tool result)
    let model = Arc::new(ScriptedChatModel::new(vec![
        ChatResponse {
            message: Message::ai_with_tool_calls(
                "",
                vec![ToolCall {
                    id: "call-1".to_string(),
                    name: "echo".to_string(),
                    arguments: serde_json::json!({"input": "test"}),
                }],
            ),
            usage: None,
        },
        ChatResponse {
            message: Message::ai("The echo result is test"),
            usage: None,
        },
    ]));

    let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(EchoTool)];
    let graph = create_react_agent(model, tools).unwrap();

    let state = MessageState::with_messages(vec![Message::human("echo test")]);
    let result = graph.invoke(state).await.unwrap();

    // Should have: human, AI (tool call), tool result, AI (final)
    assert_eq!(result.messages.len(), 4);
    assert!(result.messages[0].is_human());
    assert!(result.messages[1].is_ai());
    assert!(!result.messages[1].tool_calls().is_empty());
    assert!(result.messages[2].is_tool());
    assert!(result.messages[3].is_ai());
    assert_eq!(result.messages[3].content(), "The echo result is test");
}
