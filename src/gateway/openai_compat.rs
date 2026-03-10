//! OpenAI Chat Completions compatible API endpoint.
//!
//! Exposes POST /v1/chat/completions that accepts OpenAI-format requests
//! and returns OpenAI-format responses, using the configured model.

use std::convert::Infallible;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Json};
use axum::routing::post;
use axum::Router;
use futures::stream::Stream;
use futures::StreamExt;
use serde::{Deserialize, Serialize};
use synaptic::core::{ChatRequest, Message};

use super::state::AppState;

// ---------------------------------------------------------------------------
// Request types
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct CompletionRequest {
    model: Option<String>,
    messages: Vec<CompletionMessage>,
    /// Accepted for API compatibility; not yet forwarded to the model.
    #[serde(default)]
    #[allow(dead_code)]
    temperature: Option<f64>,
    /// Accepted for API compatibility; not yet forwarded to the model.
    #[serde(default)]
    #[allow(dead_code)]
    max_tokens: Option<u32>,
    #[serde(default)]
    stream: bool,
}

#[derive(Deserialize)]
struct CompletionMessage {
    role: String,
    content: String,
}

// ---------------------------------------------------------------------------
// Response types
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct CompletionResponse {
    id: String,
    object: String,
    created: u64,
    model: String,
    choices: Vec<Choice>,
    usage: Option<Usage>,
}

#[derive(Serialize)]
struct Choice {
    index: u32,
    message: ResponseMessage,
    finish_reason: String,
}

#[derive(Serialize)]
struct ResponseMessage {
    role: String,
    content: String,
}

#[derive(Serialize)]
struct Usage {
    prompt_tokens: u32,
    completion_tokens: u32,
    total_tokens: u32,
}

// ---------------------------------------------------------------------------
// Router
// ---------------------------------------------------------------------------

pub fn routes() -> Router<AppState> {
    Router::new().route("/v1/chat/completions", post(chat_completions))
}

// ---------------------------------------------------------------------------
// Handler
// ---------------------------------------------------------------------------

async fn chat_completions(
    State(state): State<AppState>,
    Json(req): Json<CompletionRequest>,
) -> Result<axum::response::Response, (StatusCode, String)> {
    // Build model — use override if provided, else default
    let model = if let Some(ref model_name) = req.model {
        crate::agent::build_model_by_name(&state.config, model_name)
            .map_err(|e| (StatusCode::BAD_REQUEST, format!("invalid model: {e}")))?
    } else {
        state.model.clone()
    };

    // Convert OpenAI messages to Synaptic messages
    let messages: Vec<Message> = req
        .messages
        .iter()
        .map(|m| match m.role.as_str() {
            "system" => Message::system(&m.content),
            "assistant" => Message::ai(&m.content),
            _ => Message::human(&m.content),
        })
        .collect();

    let model_name = req
        .model
        .clone()
        .unwrap_or_else(|| state.config.base.model.model.clone());

    if req.stream {
        // SSE streaming mode
        let chat_request = ChatRequest::new(messages);
        let stream = make_streaming_response(model, chat_request, model_name);
        Ok(Sse::new(stream).keep_alive(KeepAlive::default()).into_response())
    } else {
        // Non-streaming mode
        let chat_request = ChatRequest::new(messages);

        let response = model
            .chat(chat_request)
            .await
            .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, format!("model error: {e}")))?;

        let content = response.message.content().to_string();

        let usage = response.usage.map(|u| Usage {
            prompt_tokens: u.input_tokens,
            completion_tokens: u.output_tokens,
            total_tokens: u.total_tokens,
        });

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        Ok(Json(CompletionResponse {
            id: format!("chatcmpl-{}", uuid::Uuid::new_v4()),
            object: "chat.completion".to_string(),
            created: now,
            model: model_name,
            choices: vec![Choice {
                index: 0,
                message: ResponseMessage {
                    role: "assistant".to_string(),
                    content,
                },
                finish_reason: "stop".to_string(),
            }],
            usage,
        }).into_response())
    }
}

/// Streaming chunk for OpenAI SSE format.
#[derive(Serialize)]
struct StreamChunk {
    id: String,
    object: String,
    created: u64,
    model: String,
    choices: Vec<StreamChoice>,
}

#[derive(Serialize)]
struct StreamChoice {
    index: u32,
    delta: StreamDelta,
    finish_reason: Option<String>,
}

#[derive(Serialize)]
struct StreamDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
}

fn make_streaming_response(
    model: std::sync::Arc<dyn synaptic::core::ChatModel>,
    request: ChatRequest,
    model_name: String,
) -> impl Stream<Item = Result<Event, Infallible>> {
    let (tx, rx) = tokio::sync::mpsc::channel::<Event>(64);

    tokio::spawn(async move {
        let completion_id = format!("chatcmpl-{}", uuid::Uuid::new_v4());
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();

        // Send initial chunk with role
        let initial = StreamChunk {
            id: completion_id.clone(),
            object: "chat.completion.chunk".to_string(),
            created: now,
            model: model_name.clone(),
            choices: vec![StreamChoice {
                index: 0,
                delta: StreamDelta {
                    role: Some("assistant".to_string()),
                    content: None,
                },
                finish_reason: None,
            }],
        };
        let _ = tx
            .send(Event::default().data(serde_json::to_string(&initial).unwrap_or_default()))
            .await;

        // Stream content tokens
        let mut chat_stream = model.stream_chat(request);
        while let Some(chunk) = chat_stream.next().await {
            match chunk {
                Ok(c) => {
                    let chunk_data = StreamChunk {
                        id: completion_id.clone(),
                        object: "chat.completion.chunk".to_string(),
                        created: now,
                        model: model_name.clone(),
                        choices: vec![StreamChoice {
                            index: 0,
                            delta: StreamDelta {
                                role: None,
                                content: Some(c.content),
                            },
                            finish_reason: None,
                        }],
                    };
                    if tx
                        .send(Event::default().data(serde_json::to_string(&chunk_data).unwrap_or_default()))
                        .await
                        .is_err()
                    {
                        break;
                    }
                }
                Err(_) => break,
            }
        }

        // Send final chunk with finish_reason
        let final_chunk = StreamChunk {
            id: completion_id,
            object: "chat.completion.chunk".to_string(),
            created: now,
            model: model_name,
            choices: vec![StreamChoice {
                index: 0,
                delta: StreamDelta {
                    role: None,
                    content: None,
                },
                finish_reason: Some("stop".to_string()),
            }],
        };
        let _ = tx
            .send(Event::default().data(serde_json::to_string(&final_chunk).unwrap_or_default()))
            .await;

        // Send [DONE] marker
        let _ = tx.send(Event::default().data("[DONE]")).await;
    });

    tokio_stream::wrappers::ReceiverStream::new(rx).map(Ok)
}
