use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use synapse_agents::{AgentConfig, ReActAgentExecutor};
use synapse_callbacks::RecordingCallback;
use synapse_core::{
    Agent, ChatModel, ChatRequest, ChatResponse, MemoryStore, Message, SynapseError, Tool, ToolCall,
};
use synapse_memory::InMemoryStore;
use synapse_tools::{SerialToolExecutor, ToolRegistry};

struct ScriptedModel;

#[async_trait]
impl ChatModel for ScriptedModel {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, SynapseError> {
        let has_tool_result = request.messages.iter().any(|m| m.is_tool());

        if !has_tool_result {
            Ok(ChatResponse {
                message: Message::ai_with_tool_calls(
                    "calling tool",
                    vec![ToolCall {
                        id: "call-1".to_string(),
                        name: "add".to_string(),
                        arguments: json!({"a": 1, "b": 2}),
                    }],
                ),
                usage: None,
            })
        } else {
            Ok(ChatResponse {
                message: Message::ai("result is 3"),
                usage: None,
            })
        }
    }
}

struct AddTool;

#[async_trait]
impl Tool for AddTool {
    fn name(&self) -> &'static str {
        "add"
    }

    fn description(&self) -> &'static str {
        "add two numbers"
    }

    async fn call(&self, args: serde_json::Value) -> Result<serde_json::Value, SynapseError> {
        let a = args["a"].as_i64().unwrap_or_default();
        let b = args["b"].as_i64().unwrap_or_default();
        Ok(json!({"value": a + b}))
    }
}

#[tokio::test]
async fn executes_react_loop_until_final_answer() {
    let model = Arc::new(ScriptedModel);
    let memory = Arc::new(InMemoryStore::new());
    let callbacks = Arc::new(RecordingCallback::new());

    let registry = ToolRegistry::new();
    registry.register(Arc::new(AddTool)).expect("register add");
    let tool_executor = Arc::new(SerialToolExecutor::new(registry));

    let config = AgentConfig {
        system_prompt: "You are a test agent".to_string(),
        max_steps: 4,
    };

    let agent = ReActAgentExecutor::new(
        model,
        tool_executor,
        memory.clone(),
        callbacks.clone(),
        config,
    );

    let output = agent
        .run("session-1", "what is 1+2?")
        .await
        .expect("agent run should succeed");

    assert_eq!(output, "result is 3");

    let messages = memory.load("session-1").await.expect("load memory");
    assert!(messages.iter().any(|m| m.is_tool()));

    let events = callbacks.events().await;
    assert!(events.len() >= 2);
}
