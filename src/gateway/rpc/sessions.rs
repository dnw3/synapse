//! RPC handlers for session management.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use synaptic::core::MemoryStore;
use synaptic::events::{Event, EventKind};

use super::router::RpcContext;
use super::types::RpcError;

// ---------------------------------------------------------------------------
// Session overrides (shared with dashboard.rs)
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

fn load_overrides() -> SessionOverrides {
    let path = PathBuf::from(SESSION_OVERRIDES_FILE);
    if !path.exists() {
        return SessionOverrides::new();
    }
    match std::fs::read_to_string(&path) {
        Ok(content) => serde_json::from_str(&content).unwrap_or_default(),
        Err(_) => SessionOverrides::new(),
    }
}

fn save_overrides(overrides: &SessionOverrides) -> Result<(), String> {
    let path = PathBuf::from(SESSION_OVERRIDES_FILE);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    let json = serde_json::to_string_pretty(overrides).map_err(|e| e.to_string())?;
    std::fs::write(&path, json).map_err(|e| e.to_string())
}

fn parse_system_time_string(s: &str) -> String {
    if let Some(sec_start) = s.find("tv_sec: ") {
        let rest = &s[sec_start + 8..];
        if let Some(end) = rest.find(|c: char| !c.is_ascii_digit()) {
            if let Ok(secs) = rest[..end].parse::<u64>() {
                return (secs * 1000).to_string();
            }
        }
    }
    s.to_string()
}

// ---------------------------------------------------------------------------
// sessions.list
// ---------------------------------------------------------------------------

pub async fn handle_list(ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    let sessions = ctx
        .state
        .sessions
        .list_sessions()
        .await
        .map_err(|e| RpcError::internal(e.to_string()))?;

    let overrides = load_overrides();
    let memory = ctx.state.sessions.memory();
    let mut result = Vec::new();

    for s in sessions {
        let messages = memory.load(&s.id).await.unwrap_or_default();
        let count = messages.len();
        let title = messages.iter().find(|m| m.is_human()).map(|m| {
            let content = m.content();
            if content.chars().count() > 60 {
                format!("{}...", content.chars().take(60).collect::<String>())
            } else {
                content.to_string()
            }
        });
        let ovr = overrides.get(&s.id);
        result.push(json!({
            "id": s.id,
            "created_at": parse_system_time_string(&s.created_at),
            "message_count": count,
            "token_count": s.token_count,
            "compaction_count": s.compaction_count,
            "title": title,
            "label": ovr.and_then(|o| o.label.clone()),
            "thinking_level": ovr.and_then(|o| o.thinking.clone()),
            "verbose_level": ovr.and_then(|o| o.verbose.clone()),
        }));
    }

    Ok(json!(result))
}

// ---------------------------------------------------------------------------
// sessions.get
// ---------------------------------------------------------------------------

pub async fn handle_get(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let id = params
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'id' parameter"))?;

    let sessions = ctx
        .state
        .sessions
        .list_sessions()
        .await
        .map_err(|e| RpcError::internal(e.to_string()))?;

    let session = sessions
        .iter()
        .find(|s| s.id == id)
        .ok_or_else(|| RpcError::not_found(format!("session '{}' not found", id)))?;

    let memory = ctx.state.sessions.memory();
    let messages = memory.load(&session.id).await.unwrap_or_default();
    let count = messages.len();
    let title = messages.iter().find(|m| m.is_human()).map(|m| {
        let content = m.content();
        if content.chars().count() > 60 {
            format!("{}...", content.chars().take(60).collect::<String>())
        } else {
            content.to_string()
        }
    });

    let overrides = load_overrides();
    let ovr = overrides.get(id);

    Ok(json!({
        "id": session.id,
        "created_at": parse_system_time_string(&session.created_at),
        "message_count": count,
        "token_count": session.token_count,
        "compaction_count": session.compaction_count,
        "title": title,
        "label": ovr.and_then(|o| o.label.clone()),
        "thinking_level": ovr.and_then(|o| o.thinking.clone()),
        "verbose_level": ovr.and_then(|o| o.verbose.clone()),
    }))
}

// ---------------------------------------------------------------------------
// sessions.patch
// ---------------------------------------------------------------------------

pub async fn handle_patch(_ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let id = params
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'id' parameter"))?
        .to_string();

    let mut overrides = load_overrides();
    let entry = overrides.entry(id).or_default();

    if let Some(label) = params
        .get("label")
        .and_then(|v| v.as_str())
        .or_else(|| params.get("display_name").and_then(|v| v.as_str()))
    {
        if label.is_empty() {
            entry.label = None;
        } else {
            entry.label = Some(label.to_string());
        }
    }
    if let Some(thinking) = params.get("thinking").and_then(|v| v.as_str()) {
        entry.thinking = Some(thinking.to_string());
    }
    if let Some(verbose) = params.get("verbose").and_then(|v| v.as_str()) {
        entry.verbose = Some(verbose.to_string());
    }

    save_overrides(&overrides).map_err(RpcError::internal)?;

    Ok(json!({ "ok": true }))
}

// ---------------------------------------------------------------------------
// sessions.create
// ---------------------------------------------------------------------------

pub async fn handle_create(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let session_id = ctx
        .state
        .sessions
        .create_session()
        .await
        .map_err(|e| RpcError::internal(e.to_string()))?;

    // Apply optional overrides (label, thinking_level)
    let label = params.get("label").and_then(|v| v.as_str());
    let thinking_level = params.get("thinking_level").and_then(|v| v.as_str());

    if label.is_some() || thinking_level.is_some() {
        let mut overrides = load_overrides();
        let entry = overrides.entry(session_id.clone()).or_default();
        if let Some(l) = label {
            entry.label = Some(l.to_string());
        }
        if let Some(t) = thinking_level {
            entry.thinking = Some(t.to_string());
        }
        save_overrides(&overrides).map_err(RpcError::internal)?;
    }

    // Broadcast sessions.changed event
    ctx.broadcaster
        .broadcast(
            "sessions.changed",
            json!({
                "action": "created",
                "session_id": &session_id,
            }),
        )
        .await;

    Ok(json!({ "session_id": session_id, "ok": true }))
}

// ---------------------------------------------------------------------------
// sessions.delete
// ---------------------------------------------------------------------------

pub async fn handle_delete(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let id = params
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'id' parameter"))?;

    ctx.state
        .sessions
        .delete_session(id)
        .await
        .map_err(|e| RpcError::internal(e.to_string()))?;

    // Broadcast sessions.changed event
    ctx.broadcaster
        .broadcast(
            "sessions.changed",
            json!({
                "action": "deleted",
                "session_id": id,
            }),
        )
        .await;

    // Emit SessionEnd (fire-and-forget)
    {
        let event_bus = ctx.state.event_bus.clone();
        let session_id = id.to_string();
        tokio::spawn(async move {
            let mut event = Event::new(
                EventKind::SessionEnd,
                serde_json::json!({ "session_id": session_id }),
            )
            .with_source("gateway/rpc");
            let _ = event_bus.emit(&mut event).await;
        });
    }

    Ok(json!({ "ok": true }))
}

// ---------------------------------------------------------------------------
// sessions.compact
// ---------------------------------------------------------------------------

pub async fn handle_compact(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let id = params
        .get("id")
        .and_then(|v| v.as_str())
        .ok_or_else(|| RpcError::invalid_request("missing 'id' parameter"))?;

    // Broadcast compaction starting
    ctx.broadcaster
        .broadcast(
            "session.compacting",
            json!({
                "session_id": id,
                "progress": 0.0,
            }),
        )
        .await;

    // Compaction trigger — would need condenser integration
    // Broadcast compaction done (placeholder: before/after tokens both 0)
    ctx.broadcaster
        .broadcast(
            "session.compacted",
            json!({
                "session_id": id,
                "before_tokens": 0,
                "after_tokens": 0,
            }),
        )
        .await;

    Ok(json!({ "ok": true }))
}

// ---------------------------------------------------------------------------
// sessions.usage
// ---------------------------------------------------------------------------

pub async fn handle_usage(ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    let sessions = ctx
        .state
        .sessions
        .list_sessions()
        .await
        .map_err(|e| RpcError::internal(e.to_string()))?;

    let memory = ctx.state.sessions.memory();
    let mut result = Vec::new();

    for s in sessions {
        let msg_count = memory
            .load(&s.id)
            .await
            .map(|msgs| msgs.len() as u64)
            .unwrap_or(0);

        let input_tokens = (s.token_count as f64 * 0.6) as u64;
        let output_tokens = s.token_count.saturating_sub(input_tokens);

        result.push(json!({
            "session_id": s.id,
            "input_tokens": input_tokens,
            "output_tokens": output_tokens,
            "cost": 0.0,
            "request_count": msg_count / 2,
        }));
    }

    Ok(json!(result))
}

// ---------------------------------------------------------------------------
// sessions.usage.timeseries
// ---------------------------------------------------------------------------

pub async fn handle_usage_timeseries(
    ctx: Arc<RpcContext>,
    _params: Value,
) -> Result<Value, RpcError> {
    let snapshot = ctx.state.cost_tracker.snapshot().await;
    let now = chrono::Utc::now();

    let entries = if snapshot.total_requests > 0 {
        vec![json!({
            "timestamp": now.format("%Y-%m-%dT%H:00:00Z").to_string(),
            "input_tokens": snapshot.total_input_tokens,
            "output_tokens": snapshot.total_output_tokens,
            "cost": snapshot.estimated_cost_usd,
            "count": snapshot.total_requests,
        })]
    } else {
        vec![]
    };

    Ok(json!(entries))
}

// ---------------------------------------------------------------------------
// sessions.abort
// ---------------------------------------------------------------------------

pub async fn handle_abort(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let session_id = params
        .get("session_id")
        .and_then(|v| v.as_str())
        .or_else(|| params.get("id").and_then(|v| v.as_str()))
        .ok_or_else(|| RpcError::invalid_request("missing 'session_id' or 'id'"))?;

    let tokens = ctx.state.cancel_tokens.read().await;
    let aborted = if let Some(sender) = tokens.get(session_id) {
        let _ = sender.send(true);
        true
    } else {
        false
    };

    Ok(json!({ "ok": true, "aborted": aborted }))
}

// ---------------------------------------------------------------------------
// sessions.subscribe / sessions.unsubscribe
// ---------------------------------------------------------------------------

pub async fn handle_subscribe(ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    let mut subs = ctx.state.session_subscribers.write().await;
    subs.insert(ctx.conn_id.clone());
    Ok(json!({ "ok": true }))
}

pub async fn handle_unsubscribe(ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    let mut subs = ctx.state.session_subscribers.write().await;
    subs.remove(&ctx.conn_id);
    Ok(json!({ "ok": true }))
}

// ---------------------------------------------------------------------------
// sessions.preview
// ---------------------------------------------------------------------------

pub async fn handle_preview(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let session_ids: Vec<String> = params
        .get("session_ids")
        .and_then(|v| serde_json::from_value(v.clone()).ok())
        .ok_or_else(|| RpcError::invalid_request("missing 'session_ids' array"))?;
    let limit = params.get("limit").and_then(|v| v.as_u64()).unwrap_or(5) as usize;
    let max_chars = params
        .get("max_chars")
        .and_then(|v| v.as_u64())
        .unwrap_or(200) as usize;

    let memory = ctx.state.sessions.memory();
    let mut previews = Vec::new();

    for sid in &session_ids {
        let messages = memory.load(sid).await.unwrap_or_default();
        let preview_msgs: Vec<Value> = messages
            .iter()
            .rev()
            .take(limit)
            .rev()
            .map(|m| {
                let content = m.content();
                let truncated = if content.chars().count() > max_chars {
                    format!("{}...", content.chars().take(max_chars).collect::<String>())
                } else {
                    content.to_string()
                };
                json!({ "role": m.role(), "content": truncated })
            })
            .collect();
        previews.push(json!({ "session_id": sid, "messages": preview_msgs }));
    }

    Ok(json!({ "previews": previews }))
}

// ---------------------------------------------------------------------------
// sessions.reset
// ---------------------------------------------------------------------------

pub async fn handle_reset(ctx: Arc<RpcContext>, params: Value) -> Result<Value, RpcError> {
    let session_id = params
        .get("session_id")
        .and_then(|v| v.as_str())
        .or_else(|| params.get("id").and_then(|v| v.as_str()))
        .ok_or_else(|| RpcError::invalid_request("missing 'session_id' or 'id'"))?;

    let memory = ctx.state.sessions.memory();
    memory
        .clear(session_id)
        .await
        .map_err(|e| RpcError::internal(e.to_string()))?;

    // Broadcast sessions.changed event
    ctx.broadcaster
        .broadcast(
            "sessions.changed",
            json!({
                "action": "reset",
                "session_id": session_id,
            }),
        )
        .await;

    Ok(json!({ "ok": true }))
}

// ---------------------------------------------------------------------------
// sessions.usage.logs
// ---------------------------------------------------------------------------

pub async fn handle_usage_logs(ctx: Arc<RpcContext>, _params: Value) -> Result<Value, RpcError> {
    let snapshot = ctx.state.cost_tracker.snapshot().await;

    let mut per_model: Vec<Value> = snapshot
        .per_model
        .into_iter()
        .map(|(model, usage)| {
            json!({
                "model": model,
                "input_tokens": usage.input_tokens,
                "output_tokens": usage.output_tokens,
                "requests": usage.requests,
                "cost_usd": usage.cost_usd,
            })
        })
        .collect();
    per_model.sort_by(|a, b| {
        a.get("model")
            .and_then(|v| v.as_str())
            .cmp(&b.get("model").and_then(|v| v.as_str()))
    });

    Ok(json!({
        "total_input_tokens": snapshot.total_input_tokens,
        "total_output_tokens": snapshot.total_output_tokens,
        "total_requests": snapshot.total_requests,
        "total_cost_usd": snapshot.estimated_cost_usd,
        "per_model": per_model,
    }))
}
