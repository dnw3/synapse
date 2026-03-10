use axum::extract::State;
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::{delete, get, post};
use axum::Router;
use serde::Serialize;
use synaptic::core::MemoryStore;
use tracing;

use crate::gateway::state::AppState;

#[derive(Serialize)]
struct ConversationResponse {
    id: String,
    created_at: String,
    message_count: usize,
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/conversations", post(create_conversation))
        .route("/conversations", get(list_conversations))
        .route("/conversations/{id}", get(get_conversation))
        .route("/conversations/{id}", delete(delete_conversation))
}

async fn create_conversation(
    State(state): State<AppState>,
) -> Result<Json<ConversationResponse>, (StatusCode, String)> {
    let session_id = state
        .sessions
        .create_session()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    // Fetch the newly created session info for created_at
    let info = state
        .sessions
        .get_session(&session_id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?
        .unwrap_or_else(|| synaptic::session::SessionInfo {
            id: session_id.clone(),
            created_at: String::new(),
            compaction_count: 0,
            token_count: 0,
        });

    tracing::info!("conversation created");

    Ok(Json(ConversationResponse {
        id: info.id,
        created_at: info.created_at,
        message_count: 0,
    }))
}

async fn list_conversations(
    State(state): State<AppState>,
) -> Result<Json<Vec<ConversationResponse>>, (StatusCode, String)> {
    let sessions = state
        .sessions
        .list_sessions()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let memory = state.sessions.memory();
    let mut conversations = Vec::with_capacity(sessions.len());
    for s in sessions {
        let count = memory.load(&s.id).await.map(|m| m.len()).unwrap_or(0);
        conversations.push(ConversationResponse {
            id: s.id,
            created_at: s.created_at,
            message_count: count,
        });
    }

    let count = conversations.len();
    tracing::debug!(count, "conversations listed");

    Ok(Json(conversations))
}

async fn get_conversation(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<Json<ConversationResponse>, (StatusCode, String)> {
    let info = state
        .sessions
        .get_session(&id)
        .await
        .map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?
        .ok_or_else(|| (StatusCode::NOT_FOUND, format!("session '{}' not found", id)))?;

    let memory = state.sessions.memory();
    let count = memory.load(&id).await.map(|m| m.len()).unwrap_or(0);

    Ok(Json(ConversationResponse {
        id: info.id,
        created_at: info.created_at,
        message_count: count,
    }))
}

async fn delete_conversation(
    State(state): State<AppState>,
    axum::extract::Path(id): axum::extract::Path<String>,
) -> Result<StatusCode, (StatusCode, String)> {
    state
        .sessions
        .delete_session(&id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    tracing::info!(conversation_id = %id, "conversation deleted");

    Ok(StatusCode::NO_CONTENT)
}
