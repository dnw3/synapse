//! RPC handlers for chat, agent control, and polling.
//!
//! Streaming methods (`agent`, `chat.send`) are handled directly in `ws.rs`.
//! This module covers non-streaming operations: history retrieval, abort,
//! message injection, agent wait, and polling fallback.

use std::sync::Arc;

use serde_json::{json, Value};
use synaptic::core::{MemoryStore, Message};

use super::router::RpcContext;
use super::types::RpcError;

// ---------------------------------------------------------------------------
// chat.history — retrieve message history for a session
// ---------------------------------------------------------------------------

pub async fn handle_history(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    // Accept sessionKey (client-facing) or session_id (internal UUID) for backwards compat
    let session_id = if let Some(sk) = params.get("sessionKey").and_then(|v| v.as_str()) {
        // Resolve sessionKey → session_id via session list lookup
        let store_key = crate::session::key::to_store_key("default", sk);
        let sessions = ctx.state.sessions.list_sessions().await.unwrap_or_default();
        sessions
            .iter()
            .find(|s| s.session_key.as_deref() == Some(&store_key))
            .map(|s| s.session_id.clone())
            .ok_or_else(|| RpcError::invalid_request(format!("session not found: {}", sk)))?
    } else if let Some(id) = params.get("session_id").and_then(|v| v.as_str()) {
        id.to_string()
    } else {
        return Err(RpcError::invalid_request("missing 'sessionKey' parameter"));
    };

    let memory = ctx.state.sessions.memory();
    let messages = memory
        .load(&session_id)
        .await
        .map_err(|e| RpcError::internal(e.to_string()))?;

    let items: Vec<Value> = messages
        .iter()
        .map(|m| {
            let mut obj = json!({
                "role": m.role(),
                "content": m.content(),
            });
            // Include request_id if present (for LogID display)
            if let Some(rid) = m
                .additional_kwargs()
                .get("request_id")
                .and_then(|v| v.as_str())
            {
                obj["request_id"] = Value::String(rid.to_string());
            }
            // Include tool_calls if present
            let tool_calls = m.tool_calls();
            if !tool_calls.is_empty() {
                obj["tool_calls"] = json!(tool_calls
                    .iter()
                    .map(|tc| json!({ "name": tc.name, "arguments": tc.arguments }))
                    .collect::<Vec<_>>());
            }
            obj
        })
        .collect();

    // Build session_config from stored SessionInfo fields.
    let session_config = {
        let all_sessions = ctx.state.sessions.list_sessions().await.unwrap_or_default();
        if let Some(s) = all_sessions.iter().find(|s| s.session_id == session_id) {
            // Derive channel: prefer stored field, fall back to session_id prefix
            let channel = s
                .channel
                .clone()
                .or_else(|| s.chat_type.clone())
                .unwrap_or_else(|| "web".to_string());
            json!({
                "thinking_level": s.thinking_level,
                "verbose_level": s.verbose_level,
                "fast_mode": s.fast_mode.unwrap_or(false),
                "model": s.model,
                "channel": channel,
            })
        } else {
            json!({
                "thinking_level": null,
                "verbose_level": null,
                "fast_mode": false,
                "model": null,
                "channel": "web",
            })
        }
    };

    Ok(json!({
        "messages": items,
        "session_config": session_config,
    }))
}

// ---------------------------------------------------------------------------
// chat.abort — cancel a running agent for the given session
// ---------------------------------------------------------------------------

pub async fn handle_abort(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    // Accept session_id, session_key, or sessionKey — all refer to the same concept.
    let raw_key = params
        .get("session_id")
        .or_else(|| params.get("session_key"))
        .or_else(|| params.get("sessionKey"))
        .and_then(|v| v.as_str())
        .ok_or_else(|| {
            RpcError::invalid_request(
                "missing session identifier: provide 'session_id', 'session_key', or 'sessionKey'",
            )
        })?;

    // cancel_tokens are keyed by store key (e.g. "agent:default:main").
    // If the client sent a request key (e.g. "main"), convert it.
    let store_key = if raw_key.starts_with("agent:") {
        raw_key.to_string()
    } else {
        crate::session::key::to_store_key("default", raw_key)
    };

    let tokens = ctx.state.cancel_tokens.read().await;
    let aborted = if let Some(sender) = tokens.get(&store_key) {
        let _ = sender.send(true);
        tracing::info!(store_key = %store_key, "chat.abort: cancel signal sent");
        true
    } else {
        tracing::debug!(store_key = %store_key, "chat.abort: no active token for session");
        false
    };

    Ok(json!({
        "ok": true,
        "aborted": aborted,
        "session_key": raw_key,
    }))
}

// ---------------------------------------------------------------------------
// chat.inject — inject a message without triggering the agent
// ---------------------------------------------------------------------------

pub async fn handle_inject(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let session_id = params
        .get("session_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'session_id' parameter"))?;

    let role = params
        .get("role")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'role' parameter"))?;

    let content = params
        .get("content")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'content' parameter"))?;

    let message = match role {
        "human" | "user" => Message::human(content),
        "ai" | "assistant" => Message::ai(content),
        "system" => Message::system(content),
        _ => {
            return Err(RpcError::invalid_request(format!(
                "unsupported role '{role}'; expected human, ai, or system"
            )));
        }
    };

    let memory = ctx.state.sessions.memory();
    memory
        .append(session_id, message)
        .await
        .map_err(|e| RpcError::internal(e.to_string()))?;

    Ok(json!({ "ok": true }))
}

// ---------------------------------------------------------------------------
// sessions.send — store a human message and signal start of execution
// ---------------------------------------------------------------------------

pub async fn handle_session_send(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let session_id = params
        .get("session_id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'session_id'"))?;
    let message = params
        .get("message")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'message'"))?;

    // Store the human message
    let memory = ctx.state.sessions.memory();
    memory
        .append(session_id, Message::human(message))
        .await
        .map_err(|e| RpcError::internal(e.to_string()))?;

    // Broadcast session.message event
    ctx.broadcaster
        .broadcast(
            "session.message",
            json!({
                "session_id": session_id,
                "message": { "role": "human", "content": message },
            }),
        )
        .await;

    let run_id = uuid::Uuid::new_v4().to_string();
    Ok(json!({ "run_id": run_id, "status": "started" }))
}

// ---------------------------------------------------------------------------
// agent.wait — wait for a running agent to complete (placeholder)
// ---------------------------------------------------------------------------

pub async fn handle_agent_wait(_ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    // Placeholder: returns immediately as idle.
    // Future: block until the agent finishes or timeout.
    Ok(json!({ "ok": true, "status": "idle" }))
}

// ---------------------------------------------------------------------------
// poll — polling fallback for non-WebSocket clients (placeholder)
// ---------------------------------------------------------------------------

pub async fn handle_poll(_ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    // Placeholder: returns empty state.
    // Future: return buffered messages since last poll cursor.
    Ok(json!({ "messages": [], "status": "idle" }))
}
