use async_trait::async_trait;
use serde_json::json;
use synaptic_core::{MemoryStore, Message, SynapticError};

use crate::{api::bitable::BitableApi, LarkConfig};

pub struct LarkBitableMemoryStore {
    api: BitableApi,
    app_token: String,
    table_id: String,
}

impl LarkBitableMemoryStore {
    pub fn new(
        config: LarkConfig,
        app_token: impl Into<String>,
        table_id: impl Into<String>,
    ) -> Self {
        Self {
            api: BitableApi::new(config),
            app_token: app_token.into(),
            table_id: table_id.into(),
        }
    }

    pub fn app_token(&self) -> &str {
        &self.app_token
    }

    pub fn table_id(&self) -> &str {
        &self.table_id
    }
}

#[async_trait]
impl MemoryStore for LarkBitableMemoryStore {
    async fn append(&self, session_id: &str, message: Message) -> Result<(), SynapticError> {
        let role = message.role().to_string();
        let content = message.content().to_string();
        let tc_slice = message.tool_calls();
        let tool_calls = if tc_slice.is_empty() {
            String::new()
        } else {
            serde_json::to_string(tc_slice).unwrap_or_default()
        };
        let tool_call_id = message.tool_call_id().unwrap_or("").to_string();
        let seq = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis()
            .to_string();

        let records = vec![json!({
            "fields": {
                "session_id": session_id,
                "role": role,
                "content": content,
                "tool_calls": tool_calls,
                "tool_call_id": tool_call_id,
                "seq": seq,
            }
        })];
        self.api
            .batch_create_records(&self.app_token, &self.table_id, records)
            .await
            .map_err(|e| SynapticError::Memory(e.to_string()))?;
        Ok(())
    }

    async fn load(&self, session_id: &str) -> Result<Vec<Message>, SynapticError> {
        let body = json!({
            "page_size": 500,
            "filter": {
                "conjunction": "and",
                "conditions": [{
                    "field_name": "session_id",
                    "operator": "is",
                    "value": [session_id]
                }]
            },
            "sort": [{ "field_name": "seq", "desc": false }]
        });
        let items = self
            .api
            .search_records(&self.app_token, &self.table_id, body)
            .await
            .map_err(|e| SynapticError::Memory(e.to_string()))?;

        let mut messages = Vec::new();
        for item in &items {
            let f = &item["fields"];
            let role = f["role"].as_str().unwrap_or("human");
            let content = f["content"].as_str().unwrap_or("").to_string();
            let msg = match role {
                "system" => Message::system(content),
                "ai" | "assistant" => {
                    let tc_str = f["tool_calls"].as_str().unwrap_or("");
                    if tc_str.is_empty() {
                        Message::ai(content)
                    } else {
                        match serde_json::from_str(tc_str) {
                            Ok(tcs) => Message::ai_with_tool_calls(content, tcs),
                            Err(_) => Message::ai(content),
                        }
                    }
                }
                "tool" => {
                    let id = f["tool_call_id"].as_str().unwrap_or("").to_string();
                    Message::tool(id, content)
                }
                _ => Message::human(content),
            };
            messages.push(msg);
        }
        Ok(messages)
    }

    async fn clear(&self, session_id: &str) -> Result<(), SynapticError> {
        let search_body = json!({
            "page_size": 500,
            "filter": {
                "conjunction": "and",
                "conditions": [{
                    "field_name": "session_id",
                    "operator": "is",
                    "value": [session_id]
                }]
            }
        });
        let items = self
            .api
            .search_records(&self.app_token, &self.table_id, search_body)
            .await
            .map_err(|e| SynapticError::Memory(e.to_string()))?;

        let ids: Vec<String> = items
            .iter()
            .filter_map(|r| r["record_id"].as_str().map(String::from))
            .collect();

        if ids.is_empty() {
            return Ok(());
        }

        self.api
            .batch_delete_records(&self.app_token, &self.table_id, ids)
            .await
            .map_err(|e| SynapticError::Memory(e.to_string()))
    }
}
