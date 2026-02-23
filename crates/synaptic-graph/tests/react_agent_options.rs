use std::sync::Arc;

use serde_json::Value;
use synaptic_core::{ChatResponse, Message, SynapticError, Tool, ToolCall};
use synaptic_graph::{
    create_react_agent_with_options, CheckpointConfig, MessageState, ReactAgentOptions,
    StoreCheckpointer,
};
use synaptic_macros::tool;
use synaptic_models::ScriptedChatModel;

/// echoes input
#[tool(name = "echo")]
async fn echo(#[args] args: Value) -> Result<Value, SynapticError> {
    Ok(args)
}

#[test]
fn create_with_default_options_compiles() {
    let model = Arc::new(ScriptedChatModel::new(vec![]));
    let tools: Vec<Arc<dyn Tool>> = vec![echo()];
    let result = create_react_agent_with_options(model, tools, ReactAgentOptions::default());
    assert!(result.is_ok());
}

#[tokio::test]
async fn agent_with_system_prompt() {
    let model = Arc::new(ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai("I am a helpful assistant."),
        usage: None,
    }]));

    let tools: Vec<Arc<dyn Tool>> = vec![echo()];
    let options = ReactAgentOptions {
        system_prompt: Some("You are a helpful assistant.".to_string()),
        ..Default::default()
    };
    let graph = create_react_agent_with_options(model, tools, options).unwrap();

    let state = MessageState::with_messages(vec![Message::human("hi")]);
    let result = graph.invoke(state).await.unwrap().into_state();

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

    let tools: Vec<Arc<dyn Tool>> = vec![echo()];
    let options = ReactAgentOptions::default();
    let graph = create_react_agent_with_options(model, tools, options).unwrap();

    let state = MessageState::with_messages(vec![Message::human("hi")]);
    let result = graph.invoke(state).await.unwrap().into_state();

    assert_eq!(result.messages.len(), 2);
    assert_eq!(result.messages[1].content(), "Hello!");
}

#[tokio::test]
async fn agent_with_checkpointer() {
    let saver = Arc::new(StoreCheckpointer::new(Arc::new(
        synaptic_store::InMemoryStore::new(),
    )));

    let model = Arc::new(ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai("Persisted response"),
        usage: None,
    }]));

    let tools: Vec<Arc<dyn Tool>> = vec![echo()];
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
        .unwrap()
        .into_state();

    assert_eq!(result.messages.len(), 2);

    // Verify checkpoint was saved
    let saved_state: Option<MessageState> = graph.get_state(&config).await.unwrap();
    assert!(saved_state.is_some());
    let saved = saved_state.unwrap();
    assert_eq!(saved.messages.len(), 2);
}

#[tokio::test]
async fn agent_with_interrupt_before_tools() {
    let saver = Arc::new(StoreCheckpointer::new(Arc::new(
        synaptic_store::InMemoryStore::new(),
    )));

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

    let tools: Vec<Arc<dyn Tool>> = vec![echo()];
    let options = ReactAgentOptions {
        checkpointer: Some(saver.clone()),
        interrupt_before: vec!["tools".to_string()],
        ..Default::default()
    };
    let graph = create_react_agent_with_options(model, tools, options).unwrap();

    let config = CheckpointConfig::new("interrupt-thread");
    let state = MessageState::with_messages(vec![Message::human("call echo")]);
    let result = graph
        .invoke_with_config(state, Some(config.clone()))
        .await
        .unwrap();

    // Should be interrupted
    assert!(result.is_interrupted());

    // State should have been checkpointed with human + AI (tool call) messages
    let saved: MessageState = graph.get_state(&config).await.unwrap().unwrap();
    assert_eq!(saved.messages.len(), 2);
    assert!(saved.messages[0].is_human());
    assert!(saved.messages[1].is_ai());
    assert!(!saved.messages[1].tool_calls().is_empty());
}

#[tokio::test]
async fn agent_with_interrupt_after_agent() {
    let saver = Arc::new(StoreCheckpointer::new(Arc::new(
        synaptic_store::InMemoryStore::new(),
    )));

    let model = Arc::new(ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai("Response"),
        usage: None,
    }]));

    let tools: Vec<Arc<dyn Tool>> = vec![echo()];
    let options = ReactAgentOptions {
        checkpointer: Some(saver.clone()),
        interrupt_after: vec!["agent".to_string()],
        ..Default::default()
    };
    let graph = create_react_agent_with_options(model, tools, options).unwrap();

    let config = CheckpointConfig::new("interrupt-after-thread");
    let state = MessageState::with_messages(vec![Message::human("hi")]);
    let result = graph
        .invoke_with_config(state, Some(config.clone()))
        .await
        .unwrap();

    // Should be interrupted
    assert!(result.is_interrupted());
}

#[tokio::test]
async fn agent_with_tool_calls_and_system_prompt() {
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

    let tools: Vec<Arc<dyn Tool>> = vec![echo()];
    let options = ReactAgentOptions {
        system_prompt: Some("You are an echo bot.".to_string()),
        ..Default::default()
    };
    let graph = create_react_agent_with_options(model, tools, options).unwrap();

    let state = MessageState::with_messages(vec![Message::human("echo test")]);
    let result = graph.invoke(state).await.unwrap().into_state();

    // Should have: human, AI (tool call), tool result, AI (final)
    assert_eq!(result.messages.len(), 4);
    assert!(result.messages[0].is_human());
    assert!(result.messages[1].is_ai());
    assert!(!result.messages[1].tool_calls().is_empty());
    assert!(result.messages[2].is_tool());
    assert!(result.messages[3].is_ai());
    assert_eq!(result.messages[3].content(), "The echo result is test");
}
