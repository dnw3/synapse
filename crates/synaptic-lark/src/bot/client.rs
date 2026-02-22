use reqwest::Client;
use serde::Deserialize;
use serde_json::json;
use synaptic_core::SynapticError;

use crate::{api::message::MessageApi, auth::TokenCache, LarkConfig};

/// Feishu bot client: get bot info, send and reply to messages.
pub struct LarkBotClient {
    pub(crate) app_id: String,
    token_cache: TokenCache,
    base_url: String,
    client: Client,
    msg_api: MessageApi,
}

#[derive(Debug, Deserialize)]
pub struct BotInfo {
    pub app_name: String,
    pub avatar_url: String,
    pub ip_white_list: Vec<String>,
    pub open_id: String,
}

impl LarkBotClient {
    pub fn new(config: LarkConfig) -> Self {
        let app_id = config.app_id.clone();
        let base_url = config.base_url.clone();
        // MessageApi gets its own token cache (shares the same credentials).
        let msg_api = MessageApi::new(config.clone());
        Self {
            app_id,
            token_cache: config.token_cache(),
            base_url,
            client: Client::new(),
            msg_api,
        }
    }

    pub fn app_id(&self) -> &str {
        &self.app_id
    }

    /// GET /bot/v3/info
    pub async fn get_bot_info(&self) -> Result<BotInfo, SynapticError> {
        let token = self.token_cache.get_token().await?;
        let url = format!("{}/bot/v3/info", self.base_url);
        let resp: serde_json::Value = self
            .client
            .get(&url)
            .bearer_auth(token)
            .send()
            .await
            .map_err(|e| SynapticError::Tool(format!("get_bot_info: {e}")))?
            .json()
            .await
            .map_err(|e| SynapticError::Tool(format!("get_bot_info parse: {e}")))?;
        if resp["code"].as_i64().unwrap_or(-1) != 0 {
            return Err(SynapticError::Tool(format!(
                "get_bot_info error: {}",
                resp["msg"].as_str().unwrap_or("unknown")
            )));
        }
        let bot = &resp["bot"];
        Ok(BotInfo {
            app_name: bot["app_name"].as_str().unwrap_or("").to_string(),
            avatar_url: bot["avatar_url"].as_str().unwrap_or("").to_string(),
            ip_white_list: bot["ip_white_list"]
                .as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|v| v.as_str().map(String::from))
                        .collect()
                })
                .unwrap_or_default(),
            open_id: bot["open_id"].as_str().unwrap_or("").to_string(),
        })
    }

    /// Send a text message to a chat.  Returns `message_id`.
    pub async fn send_text(
        &self,
        receive_id_type: &str,
        receive_id: &str,
        text: &str,
    ) -> Result<String, SynapticError> {
        let content_json = json!({ "text": text }).to_string();
        self.msg_api
            .send(receive_id_type, receive_id, "text", &content_json)
            .await
    }

    /// Reply to a specific message in its thread.  Returns `message_id`.
    pub async fn reply_text(&self, message_id: &str, text: &str) -> Result<String, SynapticError> {
        let content_json = json!({ "text": text }).to_string();
        self.msg_api.reply(message_id, "text", &content_json).await
    }
}
