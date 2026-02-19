use serde_json::json;
use std::sync::Arc;
use synaptic_core::{ChatModel, ChatRequest, ChatResponse, Message, SynapticError};
use synaptic_deep::backend::StateBackend;
use synaptic_deep::middleware::subagent::SubAgentMiddleware;

/// A chat model that returns a scripted response (no tool calls).
struct FinalAnswerModel;

#[async_trait::async_trait]
impl ChatModel for FinalAnswerModel {
    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, SynapticError> {
        Ok(ChatResponse {
            message: Message::ai("Task completed successfully"),
            usage: None,
        })
    }
}

#[tokio::test]
async fn task_tool_basic() {
    let backend = Arc::new(StateBackend::new());
    let model: Arc<dyn ChatModel> = Arc::new(FinalAnswerModel);
    let mw = SubAgentMiddleware::new(backend, model, 3, vec![]);

    let task_tool = mw.create_task_tool();
    assert_eq!(task_tool.name(), "task");
    assert!(task_tool.parameters().is_some());
}

#[tokio::test]
async fn task_tool_spawns_subagent() {
    let backend = Arc::new(StateBackend::new());
    let model: Arc<dyn ChatModel> = Arc::new(FinalAnswerModel);
    let mw = SubAgentMiddleware::new(backend, model, 3, vec![]);

    let task_tool = mw.create_task_tool();
    let result = task_tool
        .call(json!({"description": "Say hello"}))
        .await
        .unwrap();

    assert!(result.as_str().unwrap().contains("Task completed successfully"));
}

#[tokio::test]
async fn task_tool_missing_description() {
    let backend = Arc::new(StateBackend::new());
    let model: Arc<dyn ChatModel> = Arc::new(FinalAnswerModel);
    let mw = SubAgentMiddleware::new(backend, model, 3, vec![]);

    let task_tool = mw.create_task_tool();
    let err = task_tool.call(json!({})).await;
    assert!(err.is_err());
}
