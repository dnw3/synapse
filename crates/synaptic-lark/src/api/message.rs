use serde_json::{json, Value};
use synaptic_core::SynapticError;

use crate::{auth::TokenCache, LarkConfig};

/// Lightweight internal HTTP helper for the Feishu IM (message) API.
///
/// Centralises token acquisition, URL construction, and response-code checking
/// for send / update / delete / reply operations.
pub(crate) struct MessageApi {
    token_cache: TokenCache,
    base_url: String,
    client: reqwest::Client,
}

impl MessageApi {
    pub fn new(config: LarkConfig) -> Self {
        let base_url = config.base_url.clone();
        Self {
            token_cache: config.token_cache(),
            base_url,
            client: reqwest::Client::new(),
        }
    }

    /// Send a message to a chat or user.  Returns `message_id`.
    ///
    /// `content_json` is the already-serialised JSON string that Feishu expects
    /// in the `content` field (e.g. `"{\"text\":\"hello\"}"`).
    pub async fn send(
        &self,
        receive_id_type: &str,
        receive_id: &str,
        msg_type: &str,
        content_json: &str,
    ) -> Result<String, SynapticError> {
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
        let resp: Value = self
            .client
            .post(&url)
            .bearer_auth(&token)
            .json(&body)
            .send()
            .await
            .map_err(|e| SynapticError::Tool(format!("send message: {e}")))?
            .json()
            .await
            .map_err(|e| SynapticError::Tool(format!("send message parse: {e}")))?;
        check_code(&resp, "send")?;
        Ok(resp["data"]["message_id"]
            .as_str()
            .unwrap_or("")
            .to_string())
    }

    /// Update (patch) the content of an existing message (e.g. interactive card).
    pub async fn update(
        &self,
        message_id: &str,
        msg_type: &str,
        content_json: &str,
    ) -> Result<(), SynapticError> {
        let token = self.token_cache.get_token().await?;
        let url = format!("{}/im/v1/messages/{message_id}", self.base_url);
        let body = json!({ "msg_type": msg_type, "content": content_json });
        let resp: Value = self
            .client
            .patch(&url)
            .bearer_auth(&token)
            .json(&body)
            .send()
            .await
            .map_err(|e| SynapticError::Tool(format!("update message: {e}")))?
            .json()
            .await
            .map_err(|e| SynapticError::Tool(format!("update message parse: {e}")))?;
        check_code(&resp, "update")
    }

    /// Delete (recall) a message.
    pub async fn delete(&self, message_id: &str) -> Result<(), SynapticError> {
        let token = self.token_cache.get_token().await?;
        let url = format!("{}/im/v1/messages/{message_id}", self.base_url);
        let resp: Value = self
            .client
            .delete(&url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| SynapticError::Tool(format!("delete message: {e}")))?
            .json()
            .await
            .map_err(|e| SynapticError::Tool(format!("delete message parse: {e}")))?;
        check_code(&resp, "delete")
    }

    /// Reply to an existing message in its thread.  Returns `message_id`.
    #[cfg(feature = "bot")]
    pub async fn reply(
        &self,
        message_id: &str,
        msg_type: &str,
        content_json: &str,
    ) -> Result<String, SynapticError> {
        let token = self.token_cache.get_token().await?;
        let url = format!("{}/im/v1/messages/{message_id}/reply", self.base_url);
        let body = json!({ "msg_type": msg_type, "content": content_json });
        let resp: Value = self
            .client
            .post(&url)
            .bearer_auth(&token)
            .json(&body)
            .send()
            .await
            .map_err(|e| SynapticError::Tool(format!("reply message: {e}")))?
            .json()
            .await
            .map_err(|e| SynapticError::Tool(format!("reply message parse: {e}")))?;
        check_code(&resp, "reply")?;
        Ok(resp["data"]["message_id"]
            .as_str()
            .unwrap_or("")
            .to_string())
    }
}

fn check_code(body: &Value, ctx: &str) -> Result<(), SynapticError> {
    let code = body["code"].as_i64().unwrap_or(-1);
    if code != 0 {
        Err(SynapticError::Tool(format!(
            "Lark message API error ({ctx}) code={code}: {}",
            body["msg"].as_str().unwrap_or("unknown")
        )))
    } else {
        Ok(())
    }
}
