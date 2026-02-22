use async_trait::async_trait;
use serde_json::{json, Value};
use synaptic_core::{SynapticError, Tool};

use crate::{api::message::MessageApi, LarkConfig};

/// Send, update, or delete Feishu/Lark messages as an Agent tool.
///
/// # Actions
///
/// | Action   | Description                                         |
/// |----------|-----------------------------------------------------|
/// | `send`   | Send a new message (default when `action` omitted)  |
/// | `update` | Update the content of an existing message           |
/// | `delete` | Delete (recall) a message                          |
///
/// # Tool call format — send
///
/// ```json
/// {
///   "action": "send",
///   "receive_id_type": "chat_id",
///   "receive_id": "oc_xxx",
///   "msg_type": "text",
///   "content": "Hello!"
/// }
/// ```
///
/// # Tool call format — update
///
/// ```json
/// {
///   "action": "update",
///   "message_id": "om_xxx",
///   "msg_type": "interactive",
///   "content": "{\"config\":{...}}"
/// }
/// ```
///
/// # Tool call format — delete
///
/// ```json
/// {
///   "action": "delete",
///   "message_id": "om_xxx"
/// }
/// ```
///
/// `receive_id_type` can be `"chat_id"`, `"user_id"`, `"email"`, or `"open_id"`.
/// `msg_type` can be `"text"`, `"post"`, or `"interactive"`.
/// For `"text"` the `content` field is a plain string; for other types it must be valid JSON.
pub struct LarkMessageTool {
    api: MessageApi,
}

impl LarkMessageTool {
    /// Create a new message tool.
    pub fn new(config: LarkConfig) -> Self {
        Self {
            api: MessageApi::new(config),
        }
    }
}

#[async_trait]
impl Tool for LarkMessageTool {
    fn name(&self) -> &'static str {
        "lark_send_message"
    }

    fn description(&self) -> &'static str {
        "Send, update, or delete a Feishu/Lark message. \
         Use action='send' (default) to send to a chat or user; \
         action='update' to patch an existing message; \
         action='delete' to recall a message."
    }

    fn parameters(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "Operation: send (default) | update | delete",
                    "enum": ["send", "update", "delete"]
                },
                "receive_id_type": {
                    "type": "string",
                    "description": "For 'send': type of receiver ID: chat_id | user_id | email | open_id",
                    "enum": ["chat_id", "user_id", "email", "open_id"]
                },
                "receive_id": {
                    "type": "string",
                    "description": "For 'send': the receiver ID matching receive_id_type"
                },
                "msg_type": {
                    "type": "string",
                    "description": "For 'send'/'update': message type: text | post | interactive",
                    "enum": ["text", "post", "interactive"]
                },
                "content": {
                    "type": "string",
                    "description": "For 'send'/'update': message content. For text: plain string. For post/interactive: JSON string."
                },
                "message_id": {
                    "type": "string",
                    "description": "For 'update'/'delete': the message ID (om_xxx)"
                }
            },
            "required": ["action"]
        }))
    }

    async fn call(&self, args: Value) -> Result<Value, SynapticError> {
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .unwrap_or("send");

        match action {
            "send" => {
                let receive_id_type = args["receive_id_type"]
                    .as_str()
                    .ok_or_else(|| SynapticError::Tool("missing 'receive_id_type'".to_string()))?;
                let receive_id = args["receive_id"]
                    .as_str()
                    .ok_or_else(|| SynapticError::Tool("missing 'receive_id'".to_string()))?;
                let msg_type = args["msg_type"]
                    .as_str()
                    .ok_or_else(|| SynapticError::Tool("missing 'msg_type'".to_string()))?;
                let content = args["content"]
                    .as_str()
                    .ok_or_else(|| SynapticError::Tool("missing 'content'".to_string()))?;

                let content_json = build_content_json(msg_type, content)?;
                let message_id = self
                    .api
                    .send(receive_id_type, receive_id, msg_type, &content_json)
                    .await?;
                tracing::debug!("Lark message sent: {message_id}");
                Ok(json!({ "message_id": message_id, "status": "sent" }))
            }

            "update" => {
                let message_id = args["message_id"]
                    .as_str()
                    .ok_or_else(|| SynapticError::Tool("missing 'message_id'".to_string()))?;
                let msg_type = args["msg_type"]
                    .as_str()
                    .ok_or_else(|| SynapticError::Tool("missing 'msg_type'".to_string()))?;
                let content = args["content"]
                    .as_str()
                    .ok_or_else(|| SynapticError::Tool("missing 'content'".to_string()))?;

                let content_json = build_content_json(msg_type, content)?;
                self.api.update(message_id, msg_type, &content_json).await?;
                Ok(json!({ "message_id": message_id, "status": "updated" }))
            }

            "delete" => {
                let message_id = args["message_id"]
                    .as_str()
                    .ok_or_else(|| SynapticError::Tool("missing 'message_id'".to_string()))?;
                self.api.delete(message_id).await?;
                Ok(json!({ "message_id": message_id, "status": "deleted" }))
            }

            other => Err(SynapticError::Tool(format!(
                "unknown action '{other}': expected send | update | delete"
            ))),
        }
    }
}

/// Serialise message content into the JSON string that Feishu expects.
///
/// For `"text"` the content is a plain string; for `"post"`/`"interactive"` the
/// caller must provide a valid JSON string which is validated here.
fn build_content_json(msg_type: &str, content: &str) -> Result<String, SynapticError> {
    match msg_type {
        "text" => Ok(json!({"text": content}).to_string()),
        _ => {
            serde_json::from_str::<Value>(content)
                .map_err(|e| {
                    SynapticError::Tool(format!(
                        "content is not valid JSON for msg_type='{msg_type}': {e}"
                    ))
                })?
                .to_string();
            Ok(content.to_string())
        }
    }
}
