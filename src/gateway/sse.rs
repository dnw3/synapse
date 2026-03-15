//! Server-Sent Events (SSE) streaming endpoint.
//!
//! Provides a `GET /api/chat/sse?session_id=...&message=...` endpoint that
//! returns `text/event-stream` with chunked AI responses. This is simpler
//! than WebSocket for clients that only need one-shot streaming (curl, etc.).

use std::convert::Infallible;
use std::sync::Arc;

use axum::extract::{Query, State};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use futures::stream::Stream;
use futures::StreamExt;
use serde::Deserialize;
use synaptic::core::{ChatRequest, MemoryStore, Message};
use synaptic::graph::{MessageState, StreamMode};

use crate::agent::build_deep_agent;
use crate::gateway::state::AppState;

#[derive(Deserialize)]
pub struct SseQuery {
    /// Session/conversation ID.
    pub session_id: String,
    /// User message to send.
    pub message: String,
    /// If true, use deep agent mode with tools.
    #[serde(default)]
    pub task_mode: bool,
}

pub fn routes() -> Router<AppState> {
    Router::new().route("/api/chat/sse", get(sse_chat))
}

async fn sse_chat(
    State(state): State<AppState>,
    Query(query): Query<SseQuery>,
) -> impl IntoResponse {
    let stream = make_sse_stream(state, query);
    Sse::new(stream).keep_alive(KeepAlive::default())
}

fn make_sse_stream(
    state: AppState,
    query: SseQuery,
) -> impl Stream<Item = Result<Event, Infallible>> {
    // We use an async stream via futures::stream::unfold
    let (tx, rx) = tokio::sync::mpsc::channel::<Event>(64);

    tokio::spawn(async move {
        if let Err(e) = run_sse_session(state, query, tx.clone()).await {
            let _ = tx
                .send(Event::default().event("error").data(e.to_string()))
                .await;
        }
        let _ = tx.send(Event::default().event("done").data("")).await;
    });

    tokio_stream::wrappers::ReceiverStream::new(rx).map(Ok)
}

async fn run_sse_session(
    state: AppState,
    query: SseQuery,
    tx: tokio::sync::mpsc::Sender<Event>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let memory = state.sessions.memory();

    // Save human message
    let human_msg = Message::human(&query.message);
    memory.append(&query.session_id, human_msg).await.ok();

    if query.task_mode {
        // Deep agent mode
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
        .map_err(|e| format!("failed to build agent: {}", e))?;

        let mut messages = memory.load(&query.session_id).await.unwrap_or_default();
        if !messages.iter().any(|m| m.is_system()) {
            if let Some(ref prompt) = state.config.base.agent.system_prompt {
                messages.insert(0, Message::system(prompt));
            }
        }

        let initial_state = MessageState::with_messages(messages);
        let mut graph_stream = agent.stream(initial_state, StreamMode::Values);

        let mut displayed = 0usize;

        while let Some(event) = graph_stream.next().await {
            match event {
                Ok(graph_event) => {
                    for msg in graph_event.state.messages.iter().skip(displayed) {
                        if msg.is_ai() {
                            let tool_calls = msg.tool_calls();
                            if !tool_calls.is_empty() {
                                for tc in tool_calls {
                                    let data = serde_json::json!({
                                        "name": tc.name,
                                        "args": tc.arguments,
                                    });
                                    let _ = tx
                                        .send(
                                            Event::default()
                                                .event("tool_call")
                                                .data(data.to_string()),
                                        )
                                        .await;
                                }
                            } else {
                                let content = msg.content();
                                if !content.is_empty() {
                                    let _ = tx
                                        .send(Event::default().event("token").data(content))
                                        .await;
                                }
                            }
                        } else if msg.is_tool() {
                            let content = msg.content();
                            let preview = if content.len() > 500 {
                                format!("{}...", &content[..497])
                            } else {
                                content.to_string()
                            };
                            let _ = tx
                                .send(Event::default().event("tool_result").data(preview))
                                .await;
                        }
                        displayed += 1;
                    }

                    // Save to store
                    let saved = memory
                        .load(&query.session_id)
                        .await
                        .map(|m| m.len())
                        .unwrap_or(0);
                    for msg in graph_event.state.messages.iter().skip(saved) {
                        memory.append(&query.session_id, msg.clone()).await.ok();
                    }
                }
                Err(e) => {
                    let _ = tx
                        .send(Event::default().event("error").data(e.to_string()))
                        .await;
                    break;
                }
            }
        }
    } else {
        // Simple streaming chat
        let mut messages = memory.load(&query.session_id).await.unwrap_or_default();
        if !messages.iter().any(|m| m.is_system()) {
            if let Some(ref prompt) = state.config.base.agent.system_prompt {
                messages.insert(0, Message::system(prompt));
            }
        }

        let request = ChatRequest::new(messages);
        let mut chat_stream = state.model.stream_chat(request);

        let mut full_response = String::new();
        while let Some(chunk) = chat_stream.next().await {
            match chunk {
                Ok(c) => {
                    full_response.push_str(&c.content);
                    let _ = tx
                        .send(Event::default().event("token").data(c.content))
                        .await;
                }
                Err(e) => {
                    let _ = tx
                        .send(Event::default().event("error").data(e.to_string()))
                        .await;
                    break;
                }
            }
        }

        // Save AI response
        if !full_response.is_empty() {
            let ai_msg = Message::ai(&full_response);
            memory.append(&query.session_id, ai_msg).await.ok();
        }
    }

    Ok(())
}
