use serde_json::{json, Value};
use synaptic_core::SynapticError;

use crate::{auth::TokenCache, LarkConfig};

/// Lightweight internal HTTP helper for the Feishu Bitable API.
///
/// Centralises all Bitable HTTP calls (token acquisition, URL construction,
/// response-code checking) so that the five consumers — memory store, LLM cache,
/// graph checkpointer, record loader, and agent tool — share a single
/// implementation.
pub(crate) struct BitableApi {
    token_cache: TokenCache,
    base_url: String,
    client: reqwest::Client,
}

impl BitableApi {
    pub fn new(config: LarkConfig) -> Self {
        let base_url = config.base_url.clone();
        Self {
            token_cache: config.token_cache(),
            base_url,
            client: reqwest::Client::new(),
        }
    }

    // ── Internal helpers ─────────────────────────────────────────────────────

    fn records_url(&self, app_token: &str, table_id: &str) -> String {
        format!(
            "{}/bitable/v1/apps/{app_token}/tables/{table_id}/records",
            self.base_url
        )
    }

    fn search_url(&self, app_token: &str, table_id: &str) -> String {
        format!(
            "{}/bitable/v1/apps/{app_token}/tables/{table_id}/records/search",
            self.base_url
        )
    }

    pub(crate) fn check(body: &Value, ctx: &str) -> Result<(), SynapticError> {
        let code = body["code"].as_i64().unwrap_or(-1);
        if code != 0 {
            Err(SynapticError::Tool(format!(
                "Lark Bitable API error ({ctx}) code={code}: {}",
                body["msg"].as_str().unwrap_or("unknown")
            )))
        } else {
            Ok(())
        }
    }

    // ── Record operations ─────────────────────────────────────────────────────

    /// Search records using the POST search endpoint.
    ///
    /// `body` is the full request payload (page_size, filter, sort, …).
    /// Returns the `data.items` array from the response.
    pub async fn search_records(
        &self,
        app_token: &str,
        table_id: &str,
        body: Value,
    ) -> Result<Vec<Value>, SynapticError> {
        let token = self.token_cache.get_token().await?;
        let resp = self
            .client
            .post(self.search_url(app_token, table_id))
            .bearer_auth(&token)
            .json(&body)
            .send()
            .await
            .map_err(|e| SynapticError::Tool(format!("bitable search: {e}")))?;
        let rb: Value = resp
            .json()
            .await
            .map_err(|e| SynapticError::Tool(format!("bitable search parse: {e}")))?;
        Self::check(&rb, "search")?;
        Ok(rb["data"]["items"].as_array().cloned().unwrap_or_default())
    }

    /// Batch-create records. Each element in `records` must be `{"fields": {...}}`.
    /// Returns the created record objects.
    pub async fn batch_create_records(
        &self,
        app_token: &str,
        table_id: &str,
        records: Vec<Value>,
    ) -> Result<Vec<Value>, SynapticError> {
        let token = self.token_cache.get_token().await?;
        let body = json!({ "records": records });
        let url = format!("{}/batch_create", self.records_url(app_token, table_id));
        let resp = self
            .client
            .post(&url)
            .bearer_auth(&token)
            .json(&body)
            .send()
            .await
            .map_err(|e| SynapticError::Tool(format!("bitable batch_create: {e}")))?;
        let rb: Value = resp
            .json()
            .await
            .map_err(|e| SynapticError::Tool(format!("bitable batch_create parse: {e}")))?;
        Self::check(&rb, "batch_create")?;
        Ok(rb["data"]["records"]
            .as_array()
            .cloned()
            .unwrap_or_default())
    }

    /// Update a single record's fields.
    pub async fn update_record(
        &self,
        app_token: &str,
        table_id: &str,
        record_id: &str,
        fields: Value,
    ) -> Result<(), SynapticError> {
        let token = self.token_cache.get_token().await?;
        let url = format!("{}/{record_id}", self.records_url(app_token, table_id));
        let body = json!({ "fields": fields });
        let resp = self
            .client
            .put(&url)
            .bearer_auth(&token)
            .json(&body)
            .send()
            .await
            .map_err(|e| SynapticError::Tool(format!("bitable update: {e}")))?;
        let rb: Value = resp
            .json()
            .await
            .map_err(|e| SynapticError::Tool(format!("bitable update parse: {e}")))?;
        Self::check(&rb, "update")
    }

    /// Delete a single record.
    pub async fn delete_record(
        &self,
        app_token: &str,
        table_id: &str,
        record_id: &str,
    ) -> Result<(), SynapticError> {
        let token = self.token_cache.get_token().await?;
        let url = format!("{}/{record_id}", self.records_url(app_token, table_id));
        let resp = self
            .client
            .delete(&url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| SynapticError::Tool(format!("bitable delete: {e}")))?;
        let rb: Value = resp
            .json()
            .await
            .map_err(|e| SynapticError::Tool(format!("bitable delete parse: {e}")))?;
        Self::check(&rb, "delete")
    }

    /// Batch-delete records by ID list.
    pub async fn batch_delete_records(
        &self,
        app_token: &str,
        table_id: &str,
        record_ids: Vec<String>,
    ) -> Result<(), SynapticError> {
        let token = self.token_cache.get_token().await?;
        let url = format!("{}/batch_delete", self.records_url(app_token, table_id));
        let body = json!({ "records": record_ids });
        let resp = self
            .client
            .delete(&url)
            .bearer_auth(&token)
            .json(&body)
            .send()
            .await
            .map_err(|e| SynapticError::Tool(format!("bitable batch_delete: {e}")))?;
        let rb: Value = resp
            .json()
            .await
            .map_err(|e| SynapticError::Tool(format!("bitable batch_delete parse: {e}")))?;
        Self::check(&rb, "batch_delete")
    }

    /// Batch-update records.
    ///
    /// Each element in `records` must be `{"record_id": "recXxx", "fields": {...}}`.
    pub async fn batch_update_records(
        &self,
        app_token: &str,
        table_id: &str,
        records: Vec<Value>,
    ) -> Result<(), SynapticError> {
        let token = self.token_cache.get_token().await?;
        let url = format!("{}/batch_update", self.records_url(app_token, table_id));
        let body = json!({ "records": records });
        let resp = self
            .client
            .put(&url)
            .bearer_auth(&token)
            .json(&body)
            .send()
            .await
            .map_err(|e| SynapticError::Tool(format!("bitable batch_update: {e}")))?;
        let rb: Value = resp
            .json()
            .await
            .map_err(|e| SynapticError::Tool(format!("bitable batch_update parse: {e}")))?;
        Self::check(&rb, "batch_update")
    }

    /// Paginated GET of records (used by the loader).
    ///
    /// Returns `(items, next_page_token)`.  `next_page_token` is `None` when
    /// there are no more pages.
    pub async fn list_records_page(
        &self,
        app_token: &str,
        table_id: &str,
        view_id: Option<&str>,
        page_token: Option<&str>,
    ) -> Result<(Vec<Value>, Option<String>), SynapticError> {
        let token = self.token_cache.get_token().await?;
        let mut url = format!(
            "{}/bitable/v1/apps/{app_token}/tables/{table_id}/records?page_size=100",
            self.base_url
        );
        if let Some(v) = view_id {
            url.push_str(&format!("&view_id={v}"));
        }
        if let Some(pt) = page_token {
            url.push_str(&format!("&page_token={pt}"));
        }
        let resp = self
            .client
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| SynapticError::Loader(format!("bitable page: {e}")))?;
        let body: Value = resp
            .json()
            .await
            .map_err(|e| SynapticError::Loader(format!("bitable page parse: {e}")))?;
        let code = body["code"].as_i64().unwrap_or(-1);
        if code != 0 {
            return Err(SynapticError::Loader(format!(
                "Lark Bitable API error code={code}: {}",
                body["msg"].as_str().unwrap_or("unknown")
            )));
        }
        let items = body["data"]["items"]
            .as_array()
            .cloned()
            .unwrap_or_default();
        let next = body["data"]["page_token"].as_str().map(String::from);
        let has_more = body["data"]["has_more"].as_bool().unwrap_or(false);
        Ok((items, if has_more { next } else { None }))
    }

    // ── Table management ──────────────────────────────────────────────────────

    /// List all tables in a Bitable app.
    pub async fn list_tables(&self, app_token: &str) -> Result<Vec<Value>, SynapticError> {
        let token = self.token_cache.get_token().await?;
        let url = format!("{}/bitable/v1/apps/{app_token}/tables", self.base_url);
        let resp = self
            .client
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| SynapticError::Tool(format!("bitable list_tables: {e}")))?;
        let body: Value = resp
            .json()
            .await
            .map_err(|e| SynapticError::Tool(format!("bitable list_tables parse: {e}")))?;
        Self::check(&body, "list_tables")?;
        Ok(body["data"]["items"]
            .as_array()
            .cloned()
            .unwrap_or_default())
    }

    /// Create a new table in a Bitable app.  Returns the new `table_id`.
    pub async fn create_table(&self, app_token: &str, name: &str) -> Result<String, SynapticError> {
        let token = self.token_cache.get_token().await?;
        let url = format!("{}/bitable/v1/apps/{app_token}/tables", self.base_url);
        let body = json!({ "table": { "name": name } });
        let resp = self
            .client
            .post(&url)
            .bearer_auth(&token)
            .json(&body)
            .send()
            .await
            .map_err(|e| SynapticError::Tool(format!("bitable create_table: {e}")))?;
        let rb: Value = resp
            .json()
            .await
            .map_err(|e| SynapticError::Tool(format!("bitable create_table parse: {e}")))?;
        Self::check(&rb, "create_table")?;
        Ok(rb["data"]["table_id"].as_str().unwrap_or("").to_string())
    }

    /// Delete a table from a Bitable app.
    pub async fn delete_table(&self, app_token: &str, table_id: &str) -> Result<(), SynapticError> {
        let token = self.token_cache.get_token().await?;
        let url = format!(
            "{}/bitable/v1/apps/{app_token}/tables/{table_id}",
            self.base_url
        );
        let resp = self
            .client
            .delete(&url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| SynapticError::Tool(format!("bitable delete_table: {e}")))?;
        let rb: Value = resp
            .json()
            .await
            .map_err(|e| SynapticError::Tool(format!("bitable delete_table parse: {e}")))?;
        Self::check(&rb, "delete_table")
    }

    // ── Field management ──────────────────────────────────────────────────────

    /// List all fields in a table.
    pub async fn list_fields(
        &self,
        app_token: &str,
        table_id: &str,
    ) -> Result<Vec<Value>, SynapticError> {
        let token = self.token_cache.get_token().await?;
        let url = format!(
            "{}/bitable/v1/apps/{app_token}/tables/{table_id}/fields",
            self.base_url
        );
        let resp = self
            .client
            .get(&url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| SynapticError::Tool(format!("bitable list_fields: {e}")))?;
        let body: Value = resp
            .json()
            .await
            .map_err(|e| SynapticError::Tool(format!("bitable list_fields parse: {e}")))?;
        Self::check(&body, "list_fields")?;
        Ok(body["data"]["items"]
            .as_array()
            .cloned()
            .unwrap_or_default())
    }

    /// Create a field in a table.  Returns the new `field_id`.
    ///
    /// `field_type` is the Feishu field type integer (e.g. 1 = text, 2 = number).
    pub async fn create_field(
        &self,
        app_token: &str,
        table_id: &str,
        name: &str,
        field_type: u32,
    ) -> Result<String, SynapticError> {
        let token = self.token_cache.get_token().await?;
        let url = format!(
            "{}/bitable/v1/apps/{app_token}/tables/{table_id}/fields",
            self.base_url
        );
        let body = json!({ "field_name": name, "type": field_type });
        let resp = self
            .client
            .post(&url)
            .bearer_auth(&token)
            .json(&body)
            .send()
            .await
            .map_err(|e| SynapticError::Tool(format!("bitable create_field: {e}")))?;
        let rb: Value = resp
            .json()
            .await
            .map_err(|e| SynapticError::Tool(format!("bitable create_field parse: {e}")))?;
        Self::check(&rb, "create_field")?;
        Ok(rb["data"]["field"]["field_id"]
            .as_str()
            .unwrap_or("")
            .to_string())
    }

    /// Rename a field.
    pub async fn update_field(
        &self,
        app_token: &str,
        table_id: &str,
        field_id: &str,
        name: &str,
    ) -> Result<(), SynapticError> {
        let token = self.token_cache.get_token().await?;
        let url = format!(
            "{}/bitable/v1/apps/{app_token}/tables/{table_id}/fields/{field_id}",
            self.base_url
        );
        let body = json!({ "field_name": name });
        let resp = self
            .client
            .put(&url)
            .bearer_auth(&token)
            .json(&body)
            .send()
            .await
            .map_err(|e| SynapticError::Tool(format!("bitable update_field: {e}")))?;
        let rb: Value = resp
            .json()
            .await
            .map_err(|e| SynapticError::Tool(format!("bitable update_field parse: {e}")))?;
        Self::check(&rb, "update_field")
    }

    /// Delete a field from a table.
    pub async fn delete_field(
        &self,
        app_token: &str,
        table_id: &str,
        field_id: &str,
    ) -> Result<(), SynapticError> {
        let token = self.token_cache.get_token().await?;
        let url = format!(
            "{}/bitable/v1/apps/{app_token}/tables/{table_id}/fields/{field_id}",
            self.base_url
        );
        let resp = self
            .client
            .delete(&url)
            .bearer_auth(&token)
            .send()
            .await
            .map_err(|e| SynapticError::Tool(format!("bitable delete_field: {e}")))?;
        let rb: Value = resp
            .json()
            .await
            .map_err(|e| SynapticError::Tool(format!("bitable delete_field parse: {e}")))?;
        Self::check(&rb, "delete_field")
    }
}
