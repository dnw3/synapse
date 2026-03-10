//! Agent tools for querying and managing sessions.
//!
//! `sessions_list` — list all sessions with metadata.
//! `sessions_history` — view message history for a specific session.
//! `sessions_send` — send a message to another session.
//! `sessions_spawn` — create a new session with an optional seed message.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use synaptic::core::{MemoryStore as _, SynapticError, Tool};
use synaptic::session::SessionManager;

/// Tool that allows the agent to list all sessions.
pub struct SessionsListTool {
    mgr: Arc<SessionManager>,
}

impl SessionsListTool {
    pub fn new(mgr: Arc<SessionManager>) -> Arc<dyn Tool> {
        Arc::new(Self { mgr })
    }
}

#[async_trait]
impl Tool for SessionsListTool {
    fn name(&self) -> &'static str {
        "sessions_list"
    }

    fn description(&self) -> &'static str {
        "List all chat sessions with their IDs, creation dates, and message counts."
    }

    fn parameters(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {},
            "required": []
        }))
    }

    async fn call(&self, _args: Value) -> Result<Value, SynapticError> {
        tracing::info!("session operation");
        let sessions = self.mgr.list_sessions().await?;
        if sessions.is_empty() {
            return Ok(json!("No sessions found."));
        }

        let memory = self.mgr.memory();
        let mut lines = Vec::new();
        for s in &sessions {
            let count = memory.load(&s.id).await.map(|m| m.len()).unwrap_or(0);
            lines.push(format!(
                "- {} (created: {}, messages: {})",
                s.id, s.created_at, count
            ));
        }
        Ok(json!(lines.join("\n")))
    }
}

/// Tool that allows the agent to view message history for a session.
pub struct SessionsHistoryTool {
    mgr: Arc<SessionManager>,
}

impl SessionsHistoryTool {
    pub fn new(mgr: Arc<SessionManager>) -> Arc<dyn Tool> {
        Arc::new(Self { mgr })
    }
}

#[async_trait]
impl Tool for SessionsHistoryTool {
    fn name(&self) -> &'static str {
        "sessions_history"
    }

    fn description(&self) -> &'static str {
        "View the message history of a specific session by ID."
    }

    fn parameters(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "session_id": {
                    "type": "string",
                    "description": "The session ID to view history for"
                }
            },
            "required": ["session_id"]
        }))
    }

    async fn call(&self, args: Value) -> Result<Value, SynapticError> {
        let session_id = args
            .get("session_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SynapticError::Tool("session_id is required".into()))?;

        // Verify session exists
        self.mgr
            .get_session(session_id)
            .await?
            .ok_or_else(|| SynapticError::Tool(format!("session '{}' not found", session_id)))?;

        let memory = self.mgr.memory();
        let messages = memory.load(session_id).await.unwrap_or_default();

        if messages.is_empty() {
            return Ok(json!(format!("Session {} has no messages.", session_id)));
        }

        let mut lines = Vec::new();
        lines.push(format!("Session {} ({} messages):", session_id, messages.len()));
        for (i, msg) in messages.iter().enumerate() {
            let role = if msg.is_system() {
                "system"
            } else if msg.is_human() {
                "human"
            } else if msg.is_ai() {
                "ai"
            } else if msg.is_tool() {
                "tool"
            } else {
                "unknown"
            };

            let content = msg.content();
            let short = if content.len() > 200 {
                format!("{}...", &content[..197])
            } else {
                content.to_string()
            };
            lines.push(format!("[{}] {}: {}", i, role, short));
        }
        Ok(json!(lines.join("\n")))
    }
}

/// Tool that allows the agent to send a message to another session.
pub struct SessionsSendTool {
    mgr: Arc<SessionManager>,
}

impl SessionsSendTool {
    pub fn new(mgr: Arc<SessionManager>) -> Arc<dyn Tool> {
        Arc::new(Self { mgr })
    }
}

#[async_trait]
impl Tool for SessionsSendTool {
    fn name(&self) -> &'static str {
        "sessions_send"
    }

    fn description(&self) -> &'static str {
        "Send a message to another chat session by ID. The message is appended as a human message."
    }

    fn parameters(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "session_id": {
                    "type": "string",
                    "description": "The target session ID to send the message to"
                },
                "message": {
                    "type": "string",
                    "description": "The message content to send"
                }
            },
            "required": ["session_id", "message"]
        }))
    }

    async fn call(&self, args: Value) -> Result<Value, SynapticError> {
        let session_id = args
            .get("session_id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SynapticError::Tool("session_id is required".into()))?;
        let message = args
            .get("message")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SynapticError::Tool("message is required".into()))?;

        // Verify session exists
        self.mgr
            .get_session(session_id)
            .await?
            .ok_or_else(|| SynapticError::Tool(format!("session '{}' not found", session_id)))?;

        // Append message to session memory
        let memory = self.mgr.memory();
        let msg = synaptic::core::Message::human(message);
        memory.append(session_id, msg).await?;

        Ok(json!(format!(
            "Message sent to session '{}' successfully.",
            session_id
        )))
    }
}

/// Tool that allows the agent to create (spawn) a new session.
pub struct SessionsSpawnTool {
    mgr: Arc<SessionManager>,
}

impl SessionsSpawnTool {
    pub fn new(mgr: Arc<SessionManager>) -> Arc<dyn Tool> {
        Arc::new(Self { mgr })
    }
}

#[async_trait]
impl Tool for SessionsSpawnTool {
    fn name(&self) -> &'static str {
        "sessions_spawn"
    }

    fn description(&self) -> &'static str {
        "Create a new chat session. Optionally seed it with an initial message."
    }

    fn parameters(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "task": {
                    "type": "string",
                    "description": "Optional initial message/task to seed the new session with"
                }
            },
            "required": []
        }))
    }

    async fn call(&self, args: Value) -> Result<Value, SynapticError> {
        let session_id = self.mgr.create_session().await?;

        // Optionally seed with an initial message
        if let Some(task) = args.get("task").and_then(|v| v.as_str()) {
            if !task.is_empty() {
                let memory = self.mgr.memory();
                let msg = synaptic::core::Message::human(task);
                memory.append(&session_id, msg).await?;
            }
        }

        Ok(json!({
            "session_id": session_id,
            "created": true,
            "message": format!("New session '{}' created.", session_id)
        }))
    }
}
