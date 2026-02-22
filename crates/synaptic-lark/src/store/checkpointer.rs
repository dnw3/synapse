use crate::{api::bitable::BitableApi, LarkConfig};

/// Bitable-backed graph checkpoint store.
///
/// Persists [`synaptic_graph::checkpoint::Checkpoint`] snapshots into a Feishu Bitable table,
/// enabling human-in-the-loop workflows via the Feishu UI.
///
/// The Bitable table must contain the following fields:
/// - `thread_id` (Text)
/// - `checkpoint_id` (Text)
/// - `parent_id` (Text)
/// - `state` (Text — JSON)
/// - `next_node` (Text)
/// - `metadata` (Text — JSON)
/// - `created_at` (Text — Unix timestamp string)
///
/// This struct is always compiled. The [`synaptic_graph::checkpoint::Checkpointer`] impl is
/// gated behind `#[cfg(feature = "checkpointer")]`.
#[allow(dead_code)]
pub struct LarkBitableCheckpointer {
    api: BitableApi,
    app_token: String,
    table_id: String,
}

impl LarkBitableCheckpointer {
    /// Create a new checkpointer.
    ///
    /// * `config`     — Lark application credentials and base URL.
    /// * `app_token`  — Bitable app token (e.g. `"bascnXxx"`).
    /// * `table_id`   — Table ID inside that Bitable (e.g. `"tblXxx"`).
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

    /// Return the Bitable app token this checkpointer targets.
    pub fn app_token(&self) -> &str {
        &self.app_token
    }
}

#[cfg(feature = "checkpointer")]
mod checkpointer_impl {
    use super::*;
    use async_trait::async_trait;
    use serde_json::json;
    use synaptic_core::SynapticError;
    use synaptic_graph::{Checkpoint, CheckpointConfig, Checkpointer};

    #[async_trait]
    impl Checkpointer for LarkBitableCheckpointer {
        async fn put(
            &self,
            config: &CheckpointConfig,
            checkpoint: &Checkpoint,
        ) -> Result<(), SynapticError> {
            let state_str = serde_json::to_string(&checkpoint.state)
                .map_err(|e| SynapticError::Graph(format!("serialize state: {e}")))?;
            let meta_str = serde_json::to_string(&checkpoint.metadata)
                .map_err(|e| SynapticError::Graph(format!("serialize metadata: {e}")))?;
            let records = vec![json!({
                "fields": {
                    "thread_id": &config.thread_id,
                    "checkpoint_id": &checkpoint.id,
                    "parent_id": checkpoint.parent_id.as_deref().unwrap_or(""),
                    "state": state_str,
                    "next_node": checkpoint.next_node.as_deref().unwrap_or(""),
                    "metadata": meta_str,
                    "created_at": now_ts(),
                }
            })];
            self.api
                .batch_create_records(&self.app_token, &self.table_id, records)
                .await
                .map_err(|e| SynapticError::Graph(e.to_string()))?;
            Ok(())
        }

        async fn get(
            &self,
            config: &CheckpointConfig,
        ) -> Result<Option<Checkpoint>, SynapticError> {
            let body = json!({
                "page_size": 1,
                "filter": {
                    "conjunction": "and",
                    "conditions": [{
                        "field_name": "thread_id",
                        "operator": "is",
                        "value": [&config.thread_id]
                    }]
                },
                "sort": [{ "field_name": "created_at", "desc": true }]
            });
            let items = self
                .api
                .search_records(&self.app_token, &self.table_id, body)
                .await
                .map_err(|e| SynapticError::Graph(e.to_string()))?;
            match items.into_iter().next() {
                None => Ok(None),
                Some(item) => Ok(Some(record_to_checkpoint(&item)?)),
            }
        }

        async fn list(&self, config: &CheckpointConfig) -> Result<Vec<Checkpoint>, SynapticError> {
            let body = json!({
                "page_size": 100,
                "filter": {
                    "conjunction": "and",
                    "conditions": [{
                        "field_name": "thread_id",
                        "operator": "is",
                        "value": [&config.thread_id]
                    }]
                },
                "sort": [{ "field_name": "created_at", "desc": false }]
            });
            let items = self
                .api
                .search_records(&self.app_token, &self.table_id, body)
                .await
                .map_err(|e| SynapticError::Graph(e.to_string()))?;
            items.iter().map(record_to_checkpoint).collect()
        }
    }

    fn now_ts() -> String {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs()
            .to_string()
    }

    fn record_to_checkpoint(item: &serde_json::Value) -> Result<Checkpoint, SynapticError> {
        let f = &item["fields"];
        let state: serde_json::Value = serde_json::from_str(f["state"].as_str().unwrap_or("{}"))
            .map_err(|e| SynapticError::Graph(format!("deserialize state: {e}")))?;
        let metadata: std::collections::HashMap<String, serde_json::Value> =
            serde_json::from_str(f["metadata"].as_str().unwrap_or("{}")).unwrap_or_default();
        let next_node = f["next_node"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(String::from);
        let parent_id = f["parent_id"]
            .as_str()
            .filter(|s| !s.is_empty())
            .map(String::from);
        let id = f["checkpoint_id"].as_str().unwrap_or("").to_string();
        Ok(Checkpoint {
            id,
            state,
            next_node,
            parent_id,
            metadata,
        })
    }
}
