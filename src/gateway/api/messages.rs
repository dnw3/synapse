use std::sync::Arc;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::post;
use axum::Router;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use synaptic::core::{MemoryStore, Message};
use synaptic::graph::{MessageState, StreamMode};

use tracing;

use crate::agent::build_deep_agent;
use crate::gateway::state::AppState;

#[derive(Deserialize)]
pub struct SendMessageRequest {
    pub content: String,
    /// If true, run as a deep agent task (with tools).
    #[serde(default)]
    pub task_mode: bool,
}

#[derive(Serialize)]
pub struct MessageResponse {
    pub role: String,
    pub content: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub tool_calls: Vec<ToolCallResponse>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
}

#[derive(Serialize)]
pub struct ToolCallResponse {
    pub name: String,
    pub arguments: serde_json::Value,
}

pub fn routes() -> Router<AppState> {
    Router::new().route(
        "/conversations/{id}/messages",
        post(send_message).get(get_messages),
    )
}

async fn send_message(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
    Json(body): Json<SendMessageRequest>,
) -> Result<Json<Vec<MessageResponse>>, (StatusCode, String)> {
    // Verify session exists
    state
        .sessions
        .get_session(&id)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("session '{}' not found", id)))?;

    tracing::info!(conversation_id = %id, "message received via HTTP");

    let memory = state.sessions.memory();

    let human_msg = Message::human(&body.content);
    memory
        .append(&id, human_msg)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    if body.task_mode {
        // Deep agent execution
        let cwd = std::env::current_dir().unwrap_or_else(|_| ".".into());
        let checkpointer = Arc::new(state.sessions.checkpointer());
        let agent = build_deep_agent(
            state.model.clone(),
            &state.config,
            &cwd,
            checkpointer,
            vec![],
            None,
        )
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

        let mut messages = memory.load(&id).await.unwrap_or_default();
        if !messages.iter().any(|m| m.is_system()) {
            if let Some(ref prompt) = state.config.base.agent.system_prompt {
                messages.insert(0, Message::system(prompt));
            }
        }

        let initial_state = MessageState::with_messages(messages);
        let mut stream = agent.stream(initial_state, StreamMode::Values);

        let mut final_messages = Vec::new();
        while let Some(event) = stream.next().await {
            match event {
                Ok(graph_event) => {
                    final_messages = graph_event.state.messages;
                }
                Err(e) => {
                    return Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()));
                }
            }
        }

        // Save all new messages to store
        let saved_count = memory.load(&id).await.map(|m| m.len()).unwrap_or(0);
        for msg in final_messages.iter().skip(saved_count) {
            memory.append(&id, msg.clone()).await.ok();
        }

        let response_msgs: Vec<MessageResponse> = final_messages
            .iter()
            .skip(saved_count.saturating_sub(1))
            .filter(|m| m.is_ai() || m.is_tool())
            .map(message_to_response)
            .collect();

        Ok(Json(response_msgs))
    } else {
        // Simple chat mode
        let mut messages = memory.load(&id).await.unwrap_or_default();
        if !messages.iter().any(|m| m.is_system()) {
            if let Some(ref prompt) = state.config.base.agent.system_prompt {
                messages.insert(0, Message::system(prompt));
            }
        }

        let request = synaptic::core::ChatRequest::new(messages);
        let llm_start = std::time::Instant::now();
        let response = state
            .model
            .chat(request)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
        let llm_duration = llm_start.elapsed().as_secs_f64();

        // Track token usage and LLM duration
        let model_name = state.config.base.model.model.clone();
        state.cost_tracker.set_model(&model_name).await;
        if let Some(ref usage) = response.usage {
            state.cost_tracker.record_usage(usage).await;
        }
        {
            let mut llm_durs = state.request_metrics.llm_durations.write().await;
            let entry = llm_durs.entry(model_name).or_insert((0, 0.0));
            entry.0 += 1;
            entry.1 += llm_duration;
        }

        let ai_msg = response.message;
        memory.append(&id, ai_msg.clone()).await.ok();

        Ok(Json(vec![message_to_response(&ai_msg)]))
    }
}

async fn get_messages(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<Vec<MessageResponse>>, (StatusCode, String)> {
    // Verify session exists
    state
        .sessions
        .get_session(&id)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("session '{}' not found", id)))?;

    let memory = state.sessions.memory();
    let messages = memory
        .load(&id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    tracing::debug!(conversation_id = %id, "messages loaded");

    let responses: Vec<MessageResponse> = messages.iter().map(message_to_response).collect();
    Ok(Json(responses))
}

fn message_to_response(msg: &Message) -> MessageResponse {
    let role = if msg.is_system() {
        "system"
    } else if msg.is_human() {
        "human"
    } else if msg.is_ai() {
        "assistant"
    } else if msg.is_tool() {
        "tool"
    } else {
        "unknown"
    };

    let tool_calls: Vec<ToolCallResponse> = msg
        .tool_calls()
        .iter()
        .map(|tc| ToolCallResponse {
            name: tc.name.clone(),
            arguments: tc.arguments.clone(),
        })
        .collect();

    let request_id = msg
        .additional_kwargs()
        .get("request_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    MessageResponse {
        role: role.to_string(),
        content: msg.content().to_string(),
        tool_calls,
        request_id,
    }
}
