use async_trait::async_trait;
use serde_json::Value;
use std::collections::HashMap;
use synaptic_core::{Document, Loader, SynapticError};

#[derive(Debug, Clone)]
pub struct SlackConfig {
    pub bot_token: String,
    pub channel_ids: Vec<String>,
    pub oldest: Option<String>,
    pub limit: usize,
    pub include_threads: bool,
}

impl SlackConfig {
    pub fn new(bot_token: impl Into<String>, channel_ids: Vec<String>) -> Self {
        Self {
            bot_token: bot_token.into(),
            channel_ids,
            oldest: None,
            limit: 100,
            include_threads: false,
        }
    }

    pub fn with_oldest(mut self, ts: impl Into<String>) -> Self {
        self.oldest = Some(ts.into());
        self
    }

    pub fn with_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
    }

    pub fn with_threads(mut self) -> Self {
        self.include_threads = true;
        self
    }
}

pub struct SlackLoader {
    config: SlackConfig,
    client: reqwest::Client,
}

impl SlackLoader {
    pub fn new(config: SlackConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }

    async fn fetch_messages(&self, channel_id: &str) -> Result<Vec<Value>, SynapticError> {
        let mut params = vec![
            ("channel", channel_id.to_string()),
            ("limit", self.config.limit.to_string()),
        ];
        if let Some(ref oldest) = self.config.oldest {
            params.push(("oldest", oldest.clone()));
        }

        let resp = self
            .client
            .get("https://slack.com/api/conversations.history")
            .bearer_auth(&self.config.bot_token)
            .query(&params)
            .send()
            .await
            .map_err(|e| SynapticError::Loader(format!("Slack fetch: {e}")))?;

        let body: Value = resp
            .json()
            .await
            .map_err(|e| SynapticError::Loader(format!("Slack parse: {e}")))?;

        if !body["ok"].as_bool().unwrap_or(false) {
            return Err(SynapticError::Loader(format!(
                "Slack API error: {}",
                body["error"].as_str().unwrap_or("unknown")
            )));
        }

        Ok(body["messages"].as_array().cloned().unwrap_or_default())
    }
}

#[async_trait]
impl Loader for SlackLoader {
    async fn load(&self) -> Result<Vec<Document>, SynapticError> {
        let mut documents = Vec::new();
        for channel_id in &self.config.channel_ids {
            let messages = self.fetch_messages(channel_id).await?;
            for msg in messages {
                let text = msg["text"].as_str().unwrap_or("").to_string();
                if text.is_empty() {
                    continue;
                }
                let ts = msg["ts"].as_str().unwrap_or("").to_string();
                let user = msg["user"].as_str().unwrap_or("").to_string();
                let doc_id = format!("{}-{}", channel_id, ts);

                let mut metadata = HashMap::new();
                metadata.insert("channel".to_string(), Value::String(channel_id.clone()));
                metadata.insert("ts".to_string(), Value::String(ts));
                metadata.insert("user".to_string(), Value::String(user));
                metadata.insert(
                    "source".to_string(),
                    Value::String(format!("slack:{}", channel_id)),
                );
                if let Some(thread_ts) = msg["thread_ts"].as_str() {
                    metadata.insert(
                        "thread_ts".to_string(),
                        Value::String(thread_ts.to_string()),
                    );
                }

                documents.push(Document {
                    id: doc_id,
                    content: text,
                    metadata,
                });
            }
        }
        Ok(documents)
    }
}
