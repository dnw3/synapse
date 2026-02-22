use async_trait::async_trait;
use serde_json::{json, Value};
use synaptic_core::{SynapticError, Tool};

use crate::{auth::TokenCache, LarkConfig};

/// Interact with Feishu/Lark Bitable (multi-dimensional tables) as an Agent tool.
///
/// Supports **search**, **create**, and **update** operations on table records.
///
/// # Tool call format
///
/// **Search:**
/// ```json
/// {
///   "action": "search",
///   "app_token": "bascnXxx",
///   "table_id": "tblXxx",
///   "filter": {"field": "Status", "value": "Pending"}
/// }
/// ```
///
/// **Create:**
/// ```json
/// {
///   "action": "create",
///   "app_token": "bascnXxx",
///   "table_id": "tblXxx",
///   "records": [{"Field A": "value1", "Field B": 42}]
/// }
/// ```
///
/// **Update:**
/// ```json
/// {
///   "action": "update",
///   "app_token": "bascnXxx",
///   "table_id": "tblXxx",
///   "record_id": "recXxx",
///   "fields": {"Status": "Done"}
/// }
/// ```
pub struct LarkBitableTool {
    token_cache: TokenCache,
    base_url: String,
    client: reqwest::Client,
}

impl LarkBitableTool {
    /// Create a new Bitable tool.
    pub fn new(config: LarkConfig) -> Self {
        let base_url = config.base_url.clone();
        Self {
            token_cache: config.token_cache(),
            base_url,
            client: reqwest::Client::new(),
        }
    }

    async fn search(
        &self,
        token: &str,
        app_token: &str,
        table_id: &str,
        filter: Option<&Value>,
    ) -> Result<Value, SynapticError> {
        let url = format!(
            "{}/bitable/v1/apps/{app_token}/tables/{table_id}/records/search",
            self.base_url
        );

        let body = if let Some(f) = filter {
            let field = f["field"].as_str().unwrap_or("");
            let value = &f["value"];
            json!({
                "page_size": 20,
                "filter": {
                    "conjunction": "and",
                    "conditions": [{
                        "field_name": field,
                        "operator": "is",
                        "value": [value]
                    }]
                }
            })
        } else {
            json!({ "page_size": 20 })
        };

        let resp = self
            .client
            .post(&url)
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .map_err(|e| SynapticError::Tool(format!("Lark bitable search: {e}")))?;

        let resp_body: Value = resp
            .json()
            .await
            .map_err(|e| SynapticError::Tool(format!("Lark bitable parse: {e}")))?;

        check_code(&resp_body, "search")?;

        let items = resp_body["data"]["items"].clone();
        Ok(json!({ "records": items }))
    }

    async fn create(
        &self,
        token: &str,
        app_token: &str,
        table_id: &str,
        records: &[Value],
    ) -> Result<Value, SynapticError> {
        let url = format!(
            "{}/bitable/v1/apps/{app_token}/tables/{table_id}/records/batch_create",
            self.base_url
        );
        let records_payload: Vec<Value> = records.iter().map(|r| json!({ "fields": r })).collect();
        let body = json!({ "records": records_payload });

        let resp = self
            .client
            .post(&url)
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .map_err(|e| SynapticError::Tool(format!("Lark bitable create: {e}")))?;

        let resp_body: Value = resp
            .json()
            .await
            .map_err(|e| SynapticError::Tool(format!("Lark bitable create parse: {e}")))?;

        check_code(&resp_body, "create")?;

        let created = resp_body["data"]["records"].clone();
        Ok(json!({ "created": created }))
    }

    async fn update(
        &self,
        token: &str,
        app_token: &str,
        table_id: &str,
        record_id: &str,
        fields: &Value,
    ) -> Result<Value, SynapticError> {
        let url = format!(
            "{}/bitable/v1/apps/{app_token}/tables/{table_id}/records/{record_id}",
            self.base_url
        );
        let body = json!({ "fields": fields });

        let resp = self
            .client
            .put(&url)
            .bearer_auth(token)
            .json(&body)
            .send()
            .await
            .map_err(|e| SynapticError::Tool(format!("Lark bitable update: {e}")))?;

        let resp_body: Value = resp
            .json()
            .await
            .map_err(|e| SynapticError::Tool(format!("Lark bitable update parse: {e}")))?;

        check_code(&resp_body, "update")?;

        Ok(json!({ "record_id": record_id, "status": "updated" }))
    }
}

fn check_code(body: &Value, ctx: &str) -> Result<(), SynapticError> {
    let code = body["code"].as_i64().unwrap_or(-1);
    if code != 0 {
        return Err(SynapticError::Tool(format!(
            "Lark Bitable API error ({ctx}) code={code}: {}",
            body["msg"].as_str().unwrap_or("unknown")
        )));
    }
    Ok(())
}

#[async_trait]
impl Tool for LarkBitableTool {
    fn name(&self) -> &'static str {
        "lark_bitable"
    }

    fn description(&self) -> &'static str {
        "Interact with a Feishu/Lark Bitable (multi-dimensional table). Supports search, create, and update operations on records."
    }

    fn parameters(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "Operation to perform: search | create | update",
                    "enum": ["search", "create", "update"]
                },
                "app_token": {
                    "type": "string",
                    "description": "Bitable app token (bascnXxx)"
                },
                "table_id": {
                    "type": "string",
                    "description": "Table ID within the Bitable app (tblXxx)"
                },
                "filter": {
                    "type": "object",
                    "description": "For 'search': {\"field\": \"FieldName\", \"value\": \"FilterValue\"}",
                    "properties": {
                        "field": { "type": "string" },
                        "value": {}
                    }
                },
                "records": {
                    "type": "array",
                    "description": "For 'create': array of field objects [{\"FieldName\": value}]",
                    "items": { "type": "object" }
                },
                "record_id": {
                    "type": "string",
                    "description": "For 'update': the record ID to update (recXxx)"
                },
                "fields": {
                    "type": "object",
                    "description": "For 'update': fields to update {\"FieldName\": newValue}"
                }
            },
            "required": ["action", "app_token", "table_id"]
        }))
    }

    async fn call(&self, args: Value) -> Result<Value, SynapticError> {
        let action = args["action"]
            .as_str()
            .ok_or_else(|| SynapticError::Tool("missing 'action'".to_string()))?;
        let app_token = args["app_token"]
            .as_str()
            .ok_or_else(|| SynapticError::Tool("missing 'app_token'".to_string()))?;
        let table_id = args["table_id"]
            .as_str()
            .ok_or_else(|| SynapticError::Tool("missing 'table_id'".to_string()))?;

        // Validate action-specific required fields before making any network calls.
        match action {
            "search" => {}
            "create" => {
                if args["records"].as_array().is_none() {
                    return Err(SynapticError::Tool("missing 'records' array".to_string()));
                }
            }
            "update" => {
                if args["record_id"].as_str().is_none() {
                    return Err(SynapticError::Tool("missing 'record_id'".to_string()));
                }
                if args.get("fields").is_none() {
                    return Err(SynapticError::Tool("missing 'fields'".to_string()));
                }
            }
            other => {
                return Err(SynapticError::Tool(format!(
                    "unknown action '{other}': expected search | create | update"
                )));
            }
        }

        let token = self.token_cache.get_token().await?;

        match action {
            "search" => {
                let filter = args.get("filter");
                self.search(&token, app_token, table_id, filter).await
            }
            "create" => {
                let records = args["records"].as_array().unwrap();
                self.create(&token, app_token, table_id, records).await
            }
            "update" => {
                let record_id = args["record_id"].as_str().unwrap();
                let fields = args.get("fields").unwrap();
                self.update(&token, app_token, table_id, record_id, fields)
                    .await
            }
            _ => unreachable!(),
        }
    }
}
