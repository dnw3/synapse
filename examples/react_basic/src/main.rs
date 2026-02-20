use std::sync::Arc;

use async_trait::async_trait;
use serde_json::json;
use synaptic::core::{
    ChatModel, ChatRequest, ChatResponse, Message, SynapticError, Tool, ToolCall,
};
use synaptic::graph::{create_react_agent, MessageState};
use synaptic::macros::{tool, traceable};

struct DemoModel;

#[async_trait]
impl ChatModel for DemoModel {
    async fn chat(&self, request: ChatRequest) -> Result<ChatResponse, SynapticError> {
        let has_tool_output = request.messages.iter().any(|m| m.is_tool());
        if !has_tool_output {
            Ok(ChatResponse {
                message: Message::ai_with_tool_calls(
                    "I will use a tool to calculate this.",
                    vec![ToolCall {
                        id: "call-1".to_string(),
                        name: "add".to_string(),
                        arguments: json!({ "a": 7, "b": 5 }),
                    }],
                ),
                usage: None,
            })
        } else {
            Ok(ChatResponse {
                message: Message::ai("The result is 12."),
                usage: None,
            })
        }
    }
}

/// Adds two numbers.
#[tool]
async fn add(a: i64, b: i64) -> Result<serde_json::Value, SynapticError> {
    Ok(json!({ "value": a + b }))
}

#[traceable]
#[tokio::main]
async fn main() -> Result<(), SynapticError> {
    let model = Arc::new(DemoModel);
    let tools: Vec<Arc<dyn Tool>> = vec![add()];

    let graph = create_react_agent(model, tools)?;

    let initial_state = MessageState {
        messages: vec![Message::human("What is 7 + 5?")],
    };

    let result = graph.invoke(initial_state).await?.into_state();
    let last = result.last_message().unwrap();
    println!("agent answer: {}", last.content());
    println!("message_count: {}", result.messages.len());
    Ok(())
}
