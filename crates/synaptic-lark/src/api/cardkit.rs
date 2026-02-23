use serde_json::{json, Value};
use synaptic_core::SynapticError;

use crate::{auth::TokenCache, LarkConfig};

/// Internal HTTP helper for the Feishu CardKit API.
///
/// Provides card entity creation, full update, and element-level streaming.
///
/// ## Streaming card flow
///
/// 1. [`create`] — create a card entity with `streaming_mode: true` → get `card_id`
/// 2. Send message with `card_id` via the IM message API
/// 3. [`stream_content`] — stream text to a specific element (typewriter effect)
/// 4. [`update`] — final full card update to disable `streaming_mode`
pub(crate) struct CardKitApi {
    token_cache: TokenCache,
    base_url: String,
    client: reqwest::Client,
}

impl CardKitApi {
    pub fn new(config: LarkConfig) -> Self {
        let base_url = config.base_url.clone();
        Self {
            token_cache: config.token_cache(),
            base_url,
            client: reqwest::Client::new(),
        }
    }

    /// Create a card entity.  Returns `card_id`.
    ///
    /// `card_json` is a Card JSON 2.0 structure, e.g.:
    /// ```json
    /// {
    ///   "schema": "2.0",
    ///   "config": { "update_multi": true },
    ///   "header": { "title": { "tag": "plain_text", "content": "Title" } },
    ///   "body": { "elements": [{ "tag": "markdown", "content": "..." }] }
    /// }
    /// ```
    pub async fn create(&self, card_json: &Value) -> Result<String, SynapticError> {
        let token = self.token_cache.get_token().await?;
        let url = format!("{}/cardkit/v1/cards", self.base_url);
        let body = json!({
            "type": "card_json",
            "data": card_json.to_string(),
        });
        let resp: Value = self
            .client
            .post(&url)
            .bearer_auth(&token)
            .json(&body)
            .send()
            .await
            .map_err(|e| SynapticError::Tool(format!("cardkit create: {e}")))?
            .json()
            .await
            .map_err(|e| SynapticError::Tool(format!("cardkit create parse: {e}")))?;
        check_code(&resp, "cardkit create")?;
        resp["data"]["card_id"]
            .as_str()
            .map(String::from)
            .ok_or_else(|| SynapticError::Tool("cardkit create: missing card_id".to_string()))
    }

    /// Update a card entity's full content (replaces entire card JSON).
    ///
    /// `sequence` must be strictly incrementing for each `card_id`.
    /// `card_json` is the updated Card JSON 2.0 structure.
    ///
    /// Use this for structural changes or to toggle `streaming_mode`.
    /// For incremental text streaming, prefer [`stream_content`].
    pub async fn update(
        &self,
        card_id: &str,
        sequence: i64,
        card_json: &Value,
    ) -> Result<(), SynapticError> {
        let token = self.token_cache.get_token().await?;
        let url = format!("{}/cardkit/v1/cards/{card_id}", self.base_url);
        let body = json!({
            "sequence": sequence,
            "card": {
                "type": "card_json",
                "data": card_json.to_string(),
            },
        });
        let resp: Value = self
            .client
            .put(&url)
            .bearer_auth(&token)
            .json(&body)
            .send()
            .await
            .map_err(|e| SynapticError::Tool(format!("cardkit update: {e}")))?
            .json()
            .await
            .map_err(|e| SynapticError::Tool(format!("cardkit update parse: {e}")))?;
        check_code(&resp, "cardkit update")
    }

    /// Stream text content to a specific element (typewriter effect).
    ///
    /// `element_id` targets a `markdown` or `plain_text` element in the card.
    /// `content` is the **full accumulated text** (not a delta).  If the new text
    /// is a prefix-extension of the old text, the Feishu client renders the
    /// addition with a typewriter animation.
    ///
    /// `sequence` must be strictly incrementing per `card_id`.
    pub async fn stream_content(
        &self,
        card_id: &str,
        element_id: &str,
        content: &str,
        sequence: i64,
    ) -> Result<(), SynapticError> {
        let token = self.token_cache.get_token().await?;
        let url = format!(
            "{}/cardkit/v1/cards/{card_id}/elements/{element_id}/content",
            self.base_url
        );
        let body = json!({
            "content": content,
            "sequence": sequence,
        });
        let resp: Value = self
            .client
            .put(&url)
            .bearer_auth(&token)
            .json(&body)
            .send()
            .await
            .map_err(|e| SynapticError::Tool(format!("cardkit stream_content: {e}")))?
            .json()
            .await
            .map_err(|e| SynapticError::Tool(format!("cardkit stream_content parse: {e}")))?;
        check_code(&resp, "cardkit stream_content")
    }
}

fn check_code(body: &Value, ctx: &str) -> Result<(), SynapticError> {
    let code = body["code"].as_i64().unwrap_or(-1);
    if code != 0 {
        Err(SynapticError::Tool(format!(
            "Lark CardKit API error ({ctx}) code={code}: {}",
            body["msg"].as_str().unwrap_or("unknown")
        )))
    } else {
        Ok(())
    }
}
