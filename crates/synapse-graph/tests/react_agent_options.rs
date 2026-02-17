use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use synapse_core::{ChatResponse, Message, SynapseError, Tool, ToolCall};
use synapse_graph::{
    create_react_agent_with_options, CheckpointConfig, MemorySaver, MessageState, ReactAgentOptions,
};
use synapse_models::ScriptedChatModel;

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
fn create_with_default_options_compiles() {
    let model = Arc::new(ScriptedChatModel::new(vec![]));
    let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(EchoTool)];
    let result = create_react_agent_with_options(model, tools, ReactAgentOptions::default());
    assert!(result.is_ok());
}

#[tokio::test]
async fn agent_with_system_prompt() {
    // The ScriptedChatModel returns the messages it receives, allowing us to verify
    // the system prompt was prepended.
    // For this test, we'll just verify the agent completes successfully and the
    // system prompt doesn't break anything.
    let model = Arc::new(ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai("I am a helpful assistant."),
        usage: None,
    }]));

    let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(EchoTool)];
    let options = ReactAgentOptions {
        system_prompt: Some("You are a helpful assistant.".to_string()),
        ..Default::default()
    };
    let graph = create_react_agent_with_options(model, tools, options).unwrap();

    let state = MessageState::with_messages(vec![Message::human("hi")]);
    let result = graph.invoke(state).await.unwrap();

    // Should have: human message + AI response
    assert_eq!(result.messages.len(), 2);
    assert!(result.messages[0].is_human());
    assert!(result.messages[1].is_ai());
    assert_eq!(result.messages[1].content(), "I am a helpful assistant.");
}

#[tokio::test]
async fn agent_without_system_prompt() {
    let model = Arc::new(ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai("Hello!"),
        usage: None,
    }]));

    let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(EchoTool)];
    let options = ReactAgentOptions::default();
    let graph = create_react_agent_with_options(model, tools, options).unwrap();

    let state = MessageState::with_messages(vec![Message::human("hi")]);
    let result = graph.invoke(state).await.unwrap();

    assert_eq!(result.messages.len(), 2);
    assert_eq!(result.messages[1].content(), "Hello!");
}

#[tokio::test]
async fn agent_with_checkpointer() {
    let saver = Arc::new(MemorySaver::new());

    let model = Arc::new(ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai("Persisted response"),
        usage: None,
    }]));

    let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(EchoTool)];
    let options = ReactAgentOptions {
        checkpointer: Some(saver.clone()),
        ..Default::default()
    };
    let graph = create_react_agent_with_options(model, tools, options).unwrap();

    let config = CheckpointConfig::new("test-thread");
    let state = MessageState::with_messages(vec![Message::human("hi")]);
    let result = graph
        .invoke_with_config(state, Some(config.clone()))
        .await
        .unwrap();

    assert_eq!(result.messages.len(), 2);

    // Verify checkpoint was saved
    let saved_state: Option<MessageState> = graph.get_state(&config).await.unwrap();
    assert!(saved_state.is_some());
    let saved = saved_state.unwrap();
    assert_eq!(saved.messages.len(), 2);
}

#[tokio::test]
async fn agent_with_interrupt_before_tools() {
    let saver = Arc::new(MemorySaver::new());

    // Model returns a tool call -> should trigger interrupt before "tools" node
    let model = Arc::new(ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai_with_tool_calls(
            "",
            vec![ToolCall {
                id: "call-1".to_string(),
                name: "echo".to_string(),
                arguments: serde_json::json!({"input": "test"}),
            }],
        ),
        usage: None,
    }]));

    let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(EchoTool)];
    let options = ReactAgentOptions {
        checkpointer: Some(saver.clone()),
        interrupt_before: vec!["tools".to_string()],
        ..Default::default()
    };
    let graph = create_react_agent_with_options(model, tools, options).unwrap();

    let config = CheckpointConfig::new("interrupt-thread");
    let state = MessageState::with_messages(vec![Message::human("call echo")]);
    let result = graph.invoke_with_config(state, Some(config.clone())).await;

    // Should fail with interrupt error
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("interrupted before node 'tools'"),
        "got: {}",
        err
    );

    // State should have been checkpointed with human + AI (tool call) messages
    let saved: MessageState = graph.get_state(&config).await.unwrap().unwrap();
    assert_eq!(saved.messages.len(), 2);
    assert!(saved.messages[0].is_human());
    assert!(saved.messages[1].is_ai());
    assert!(!saved.messages[1].tool_calls().is_empty());
}

#[tokio::test]
async fn agent_with_interrupt_after_agent() {
    let saver = Arc::new(MemorySaver::new());

    let model = Arc::new(ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai("Response"),
        usage: None,
    }]));

    let tools: Vec<Arc<dyn Tool>> = vec![Arc::new(EchoTool)];
    let options = ReactAgentOptions {
        checkpointer: Some(saver.clone()),
        interrupt_after: vec!["agent".to_string()],
        ..Default::default()
    };
    let graph = create_react_agent_with_options(model, tools, options).unwrap();

    let config = CheckpointConfig::new("interrupt-after-thread");
    let state = MessageState::with_messages(vec![Message::human("hi")]);
    let result = graph.invoke_with_config(state, Some(config.clone())).await;

    // Should fail with interrupt error
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert!(
        err.to_string().contains("interrupted after node 'agent'"),
        "got: {}",
        err
    );
}

#[tokio::test]
async fn agent_with_tool_calls_and_system_prompt() {
    // Full cycle: system prompt + tool call + tool execution + final response
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
    let options = ReactAgentOptions {
        system_prompt: Some("You are an echo bot.".to_string()),
        ..Default::default()
    };
    let graph = create_react_agent_with_options(model, tools, options).unwrap();

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
