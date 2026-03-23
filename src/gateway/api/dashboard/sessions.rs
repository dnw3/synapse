use std::collections::HashMap;
use std::path::PathBuf;

use axum::extract::{self, State};
use axum::http::StatusCode;
use axum::response::Json;
use axum::routing::{delete, get, patch, post};
use axum::Router;
use serde::{Deserialize, Serialize};
use synaptic::core::MemoryStore;

use super::{parse_system_time_string, OkResponse};
use crate::gateway::state::AppState;

/// Parse a legacy session key to extract channel/kind/display_name.
///
/// Legacy keys follow patterns like `agent:default:main`, `agent:default:lark:dm:user123`, etc.
fn parse_session_channel(id: &str) -> (String, String, String) {
    if !id.starts_with("agent:") {
        return ("web".to_string(), "web".to_string(), String::new());
    }
    let parts: Vec<&str> = id.split(':').collect();
    if parts.len() == 3 && parts[2] == "main" {
        return ("web".to_string(), "main".to_string(), "main".to_string());
    }
    for (i, part) in parts.iter().enumerate() {
        if *part == "dm" && i >= 2 {
            let channel = parts[2].to_string();
            let peer = if i + 1 < parts.len() {
                parts[i + 1]
            } else {
                ""
            };
            return (channel, "dm".to_string(), peer.to_string());
        }
        if *part == "grp" && i >= 2 {
            let channel = parts[2].to_string();
            let peer = if i + 1 < parts.len() {
                parts[i + 1..].join(":")
            } else {
                String::new()
            };
            return (channel, "group".to_string(), peer);
        }
    }
    ("web".to_string(), "web".to_string(), String::new())
}

pub fn routes() -> Router<AppState> {
    Router::new()
        .route("/dashboard/sessions", get(get_sessions))
        .route("/dashboard/sessions/{id}", delete(delete_session))
        .route("/dashboard/sessions/{id}", patch(patch_session))
        .route("/dashboard/sessions/{id}/compact", post(compact_session))
}

// ---------------------------------------------------------------------------
// Session overrides storage (file-based)
// ---------------------------------------------------------------------------

const SESSION_OVERRIDES_FILE: &str = "data/session_overrides.json";

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
struct SessionOverride {
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    verbose: Option<String>,
}

type SessionOverrides = HashMap<String, SessionOverride>;

fn overrides_path() -> PathBuf {
    PathBuf::from(SESSION_OVERRIDES_FILE)
}

fn load_overrides() -> SessionOverrides {
    let path = overrides_path();
    if !path.exists() {
        return SessionOverrides::new();
    }
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => SessionOverrides::new(),
    }
}

fn save_overrides(overrides: &SessionOverrides) -> Result<(), String> {
    let path = overrides_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(overrides).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

// ---------------------------------------------------------------------------
// GET /api/dashboard/sessions
// ---------------------------------------------------------------------------

#[derive(Serialize)]
struct SessionResponse {
    key: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    channel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    kind: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    display_name: Option<String>,
    created_at: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    updated_at: Option<String>,
    message_count: usize,
    token_count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    model: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    thinking_level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    verbose_level: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    fast_mode: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reasoning_level: Option<String>,
}

async fn get_sessions(
    State(state): State<AppState>,
) -> Result<Json<Vec<SessionResponse>>, (StatusCode, String)> {
    let sessions = state
        .session
        .sessions
        .list_sessions()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;

    let overrides = load_overrides();
    let memory = state.session.sessions.memory();
    let model_name = state.core.config.model_config().model.clone();
    let mut result = Vec::with_capacity(sessions.len());
    for s in sessions {
        let messages = memory.load(&s.session_id).await.unwrap_or_default();
        let count = messages.len();
        let (channel, kind, display_name) = if s.channel.is_some() || s.chat_type.is_some() {
            (
                s.channel.clone(),
                s.chat_type.clone(),
                s.display_name.clone(),
            )
        } else {
            let (ch, k, dn) = parse_session_channel(&s.session_id);
            (
                if ch.is_empty() { None } else { Some(ch) },
                if k.is_empty() { None } else { Some(k) },
                if dn.is_empty() { None } else { Some(dn) },
            )
        };
        let ovr = overrides.get(&s.session_id);
        let key = s.session_key.clone().unwrap_or(s.session_id);
        result.push(SessionResponse {
            key,
            channel,
            kind,
            display_name,
            created_at: parse_system_time_string(&s.created_at),
            updated_at: if s.updated_at > 0 {
                Some(s.updated_at.to_string())
            } else {
                None
            },
            message_count: count,
            token_count: s.total_tokens,
            label: ovr.and_then(|o| o.label.clone()),
            model: Some(model_name.clone()),
            thinking_level: ovr.and_then(|o| o.thinking.clone()),
            verbose_level: ovr.and_then(|o| o.verbose.clone()),
            fast_mode: None,
            reasoning_level: None,
        });
    }

    Ok(Json(result))
}

// ---------------------------------------------------------------------------
// DELETE /api/dashboard/sessions/{id}
// ---------------------------------------------------------------------------

async fn delete_session(
    State(state): State<AppState>,
    extract::Path(id): extract::Path<String>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    state
        .session
        .sessions
        .delete_session(&id)
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok(Json(OkResponse { ok: true }))
}

// ---------------------------------------------------------------------------
// PATCH /api/dashboard/sessions/{id}
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct PatchSessionRequest {
    display_name: Option<String>,
    label: Option<String>,
    thinking: Option<String>,
    verbose: Option<String>,
}

async fn patch_session(
    State(_state): State<AppState>,
    extract::Path(id): extract::Path<String>,
    Json(body): Json<PatchSessionRequest>,
) -> Result<Json<OkResponse>, (StatusCode, String)> {
    let mut overrides = load_overrides();
    let entry = overrides.entry(id).or_default();

    if let Some(label) = body.label.or(body.display_name) {
        if label.is_empty() {
            entry.label = None;
        } else {
            entry.label = Some(label);
        }
    }
    if let Some(thinking) = body.thinking {
        entry.thinking = Some(thinking);
    }
    if let Some(verbose) = body.verbose {
        entry.verbose = Some(verbose);
    }

    save_overrides(&overrides).map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    Ok(Json(OkResponse { ok: true }))
}

// ---------------------------------------------------------------------------
// POST /api/dashboard/sessions/{id}/compact
// ---------------------------------------------------------------------------

async fn compact_session(
    State(_state): State<AppState>,
    extract::Path(_id): extract::Path<String>,
) -> Json<OkResponse> {
    Json(OkResponse { ok: true })
}
