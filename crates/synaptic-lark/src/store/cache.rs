use async_trait::async_trait;
use serde_json::json;
use synaptic_core::{ChatResponse, LlmCache, SynapticError};

use crate::{api::bitable::BitableApi, LarkConfig};

/// A team-shared LLM response cache stored in a Feishu Bitable table.
///
/// Each row represents one cached response, keyed by `cache_key`. Hit counts
/// are tracked in a `hit_count` field and are visible directly in the Feishu
/// spreadsheet, making cache utilisation observable without additional tooling.
///
/// # Bitable table schema
///
/// | Field name      | Type   | Notes                         |
/// |-----------------|--------|-------------------------------|
/// | `cache_key`     | Text   | Unique cache key              |
/// | `response_json` | Text   | Serialised `ChatResponse`     |
/// | `hit_count`     | Text   | Number of cache hits (string) |
/// | `created_at`    | Text   | Unix timestamp (seconds)      |
pub struct LarkBitableLlmCache {
    api: BitableApi,
    app_token: String,
    table_id: String,
}

impl LarkBitableLlmCache {
    /// Create a new cache backed by the given Bitable table.
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

    /// Return the Bitable application token.
    pub fn app_token(&self) -> &str {
        &self.app_token
    }

    /// Return the Bitable table ID.
    pub fn table_id(&self) -> &str {
        &self.table_id
    }
}

#[async_trait]
impl LlmCache for LarkBitableLlmCache {
    async fn get(&self, key: &str) -> Result<Option<ChatResponse>, SynapticError> {
        let body = json!({
            "page_size": 1,
            "filter": {
                "conjunction": "and",
                "conditions": [{
                    "field_name": "cache_key",
                    "operator": "is",
                    "value": [key]
                }]
            }
        });
        let items = self
            .api
            .search_records(&self.app_token, &self.table_id, body)
            .await
            .map_err(|e| SynapticError::Cache(e.to_string()))?;

        let rec = match items.into_iter().next() {
            None => return Ok(None),
            Some(r) => r,
        };

        let json_str = rec["fields"]["response_json"].as_str().unwrap_or("{}");
        let response: ChatResponse = serde_json::from_str(json_str)
            .map_err(|e| SynapticError::Cache(format!("deserialize cache: {e}")))?;

        // Increment hit_count — fire-and-forget; errors ignored so a counter
        // update failure never breaks the caller.
        let record_id = rec["record_id"].as_str().unwrap_or("").to_string();
        let hit = rec["fields"]["hit_count"]
            .as_str()
            .and_then(|s| s.parse::<u64>().ok())
            .unwrap_or(0)
            + 1;
        let _ = self
            .api
            .update_record(
                &self.app_token,
                &self.table_id,
                &record_id,
                json!({ "hit_count": hit.to_string() }),
            )
            .await;

        Ok(Some(response))
    }

    async fn put(&self, key: &str, response: &ChatResponse) -> Result<(), SynapticError> {
        let json_str = serde_json::to_string(response)
            .map_err(|e| SynapticError::Cache(format!("serialize cache: {e}")))?;
        let records = vec![json!({
            "fields": {
                "cache_key": key,
                "response_json": json_str,
                "hit_count": "0",
                "created_at": now_ts(),
            }
        })];
        self.api
            .batch_create_records(&self.app_token, &self.table_id, records)
            .await
            .map_err(|e| SynapticError::Cache(e.to_string()))?;
        Ok(())
    }

    async fn clear(&self) -> Result<(), SynapticError> {
        let body = json!({ "page_size": 500 });
        let items = self
            .api
            .search_records(&self.app_token, &self.table_id, body)
            .await
            .map_err(|e| SynapticError::Cache(e.to_string()))?;

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
            .map_err(|e| SynapticError::Cache(e.to_string()))
    }
}

fn now_ts() -> String {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .to_string()
}
