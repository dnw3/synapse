use std::sync::Arc;

use serde_json::{json, Value};
use synaptic_core::{ChatResponse, Message, SynapticError, Tool, ToolCall};
use synaptic_graph::{
    create_react_agent, create_react_agent_with_options, MessageState, ReactAgentOptions,
};
use synaptic_macros::tool;
use synaptic_models::ScriptedChatModel;

/// echoes input
#[tool(name = "echo")]
async fn echo(#[args] args: Value) -> Result<Value, SynapticError> {
    Ok(args)
}

#[tokio::test]
async fn empty_state_messages() {
    // Agent with empty initial messages
    let model = Arc::new(ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai("Response to nothing"),
        usage: None,
    }]));

    let tools: Vec<Arc<dyn Tool>> = vec![echo()];
    let graph = create_react_agent(model, tools).unwrap();

    let state = MessageState::with_messages(vec![]);
    let result = graph.invoke(state).await.unwrap().into_state();
    // Should still get a response
    assert!(!result.messages.is_empty());
}

#[tokio::test]
async fn multiple_tool_calls_in_one_response() {
    // Model returns AI with two tool calls in a single response
    let model = Arc::new(ScriptedChatModel::new(vec![
        ChatResponse {
            message: Message::ai_with_tool_calls(
                "",
                vec![
                    ToolCall {
                        id: "c1".to_string(),
                        name: "echo".to_string(),
                        arguments: json!({"a": 1}),
                    },
                    ToolCall {
                        id: "c2".to_string(),
                        name: "echo".to_string(),
                        arguments: json!({"b": 2}),
                    },
                ],
            ),
            usage: None,
        },
        ChatResponse {
            message: Message::ai("Both tools executed"),
            usage: None,
        },
    ]));

    let tools: Vec<Arc<dyn Tool>> = vec![echo()];
    let graph = create_react_agent(model, tools).unwrap();

    let state = MessageState::with_messages(vec![Message::human("run two tools")]);
    let result = graph.invoke(state).await.unwrap().into_state();

    // Should have: human, AI (2 tool calls), 2 tool results, AI final
    assert!(result.messages.len() >= 5);
}

#[test]
fn create_react_agent_with_options_compiles() {
    let model = Arc::new(ScriptedChatModel::new(vec![]));
    let tools: Vec<Arc<dyn Tool>> = vec![echo()];
    let options = ReactAgentOptions {
        system_prompt: Some("You are a test agent.".to_string()),
        ..Default::default()
    };
    let result = create_react_agent_with_options(model, tools, options);
    assert!(result.is_ok());
}

#[tokio::test]
async fn agent_with_system_prompt_completes() {
    let model = Arc::new(ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai("I'm a helpful bot"),
        usage: None,
    }]));

    let tools: Vec<Arc<dyn Tool>> = vec![echo()];
    let options = ReactAgentOptions {
        system_prompt: Some("You are very helpful.".to_string()),
        ..Default::default()
    };
    let graph = create_react_agent_with_options(model, tools, options).unwrap();

    let state = MessageState::with_messages(vec![Message::human("hi")]);
    let result = graph.invoke(state).await.unwrap().into_state();

    // Agent should complete successfully with system prompt configured
    assert!(result.messages.iter().any(|m| m.is_ai()));
    assert_eq!(
        result.messages.last().unwrap().content(),
        "I'm a helpful bot"
    );
}

#[tokio::test]
async fn model_error_propagates() {
    // ScriptedChatModel with no responses will error
    let model = Arc::new(ScriptedChatModel::new(vec![]));
    let tools: Vec<Arc<dyn Tool>> = vec![echo()];
    let graph = create_react_agent(model, tools).unwrap();

    let state = MessageState::with_messages(vec![Message::human("hello")]);
    let err = graph.invoke(state).await.unwrap_err();
    assert!(err.to_string().contains("exhausted"));
}

#[tokio::test]
async fn single_message_state() {
    let model = Arc::new(ScriptedChatModel::new(vec![ChatResponse {
        message: Message::ai("Got it"),
        usage: None,
    }]));
    let tools: Vec<Arc<dyn Tool>> = vec![echo()];
    let graph = create_react_agent(model, tools).unwrap();

    let state = MessageState::with_messages(vec![Message::human("one message")]);
    let result = graph.invoke(state).await.unwrap().into_state();

    assert_eq!(result.messages.len(), 2);
    assert_eq!(result.messages[0].content(), "one message");
    assert_eq!(result.messages[1].content(), "Got it");
}
