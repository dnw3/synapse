use async_trait::async_trait;
use serde_json::{json, Value};
use synaptic_core::{SynapticError, Tool};

use crate::{auth::TokenCache, LarkConfig};

/// Send messages to Feishu/Lark chats or users as an Agent tool.
///
/// Supports `text`, `post` (rich text), and `interactive` (card JSON) message types.
///
/// # Tool call format
///
/// ```json
/// {
///   "receive_id_type": "chat_id",
///   "receive_id": "oc_xxx",
///   "msg_type": "text",
///   "content": "Hello!"
/// }
/// ```
///
/// `receive_id_type` can be `"chat_id"`, `"user_id"`, `"email"`, or `"open_id"`.
/// `msg_type` can be `"text"`, `"post"`, or `"interactive"`.
/// For `"text"` the `content` field is a plain string; for other types it must be valid JSON.
pub struct LarkMessageTool {
    token_cache: TokenCache,
    base_url: String,
    client: reqwest::Client,
}

impl LarkMessageTool {
    /// Create a new message tool.
    pub fn new(config: LarkConfig) -> Self {
        let base_url = config.base_url.clone();
        Self {
            token_cache: config.token_cache(),
            base_url,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl Tool for LarkMessageTool {
    fn name(&self) -> &'static str {
        "lark_send_message"
    }

    fn description(&self) -> &'static str {
        "Send a message to a Feishu/Lark chat or user. Supports text, rich-text (post), and interactive card formats."
    }

    fn parameters(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "receive_id_type": {
                    "type": "string",
                    "description": "Type of receiver ID: chat_id | user_id | email | open_id",
                    "enum": ["chat_id", "user_id", "email", "open_id"]
                },
                "receive_id": {
                    "type": "string",
                    "description": "The receiver ID matching receive_id_type"
                },
                "msg_type": {
                    "type": "string",
                    "description": "Message type: text | post | interactive",
                    "enum": ["text", "post", "interactive"]
                },
                "content": {
                    "type": "string",
                    "description": "Message content. For text: plain string. For post/interactive: JSON string."
                }
            },
            "required": ["receive_id_type", "receive_id", "msg_type", "content"]
        }))
    }

    async fn call(&self, args: Value) -> Result<Value, SynapticError> {
        // Validate all required fields before any network call.
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

        // Lark API requires content to be a JSON string of the message body.
        let content_json = match msg_type {
            "text" => json!({"text": content}).to_string(),
            _ => {
                // For post/interactive, content is already expected to be a JSON string.
                // Validate it parses, then pass through.
                serde_json::from_str::<Value>(content)
                    .map_err(|e| {
                        SynapticError::Tool(format!(
                            "content is not valid JSON for msg_type='{msg_type}': {e}"
                        ))
                    })?
                    .to_string()
            }
        };

        let token = self.token_cache.get_token().await?;
        let url = format!(
            "{}/im/v1/messages?receive_id_type={receive_id_type}",
            self.base_url
        );
        let body = json!({
            "receive_id": receive_id,
            "msg_type": msg_type,
            "content": content_json,
        });

        let resp = self
            .client
            .post(&url)
            .bearer_auth(&token)
            .json(&body)
            .send()
            .await
            .map_err(|e| SynapticError::Tool(format!("Lark send message: {e}")))?;

        let resp_body: Value = resp
            .json()
            .await
            .map_err(|e| SynapticError::Tool(format!("Lark message parse: {e}")))?;

        let code = resp_body["code"].as_i64().unwrap_or(-1);
        if code != 0 {
            return Err(SynapticError::Tool(format!(
                "Lark API error (send_message) code={code}: {}",
                resp_body["msg"].as_str().unwrap_or("unknown")
            )));
        }

        let message_id = resp_body["data"]["message_id"]
            .as_str()
            .unwrap_or("")
            .to_string();
        tracing::debug!("Lark message sent: {message_id}");
        Ok(json!({"message_id": message_id, "status": "sent"}))
    }
}
