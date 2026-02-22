use async_trait::async_trait;
use serde_json::{json, Value};
use synaptic_core::{SynapticError, Tool};

use crate::{api::bitable::BitableApi, LarkConfig};

/// Interact with Feishu/Lark Bitable (multi-dimensional tables) as an Agent tool.
///
/// # Supported actions
///
/// | Action          | Description                                      |
/// |-----------------|--------------------------------------------------|
/// | `search`        | Search records with optional filter              |
/// | `create`        | Batch-create new records                         |
/// | `update`        | Update a single record                           |
/// | `delete`        | Delete a single record                           |
/// | `batch_update`  | Update multiple records in one call              |
/// | `batch_delete`  | Delete multiple records in one call              |
/// | `list_tables`   | List all tables in the app                       |
/// | `list_fields`   | List fields in a table                           |
/// | `create_table`  | Create a new table                               |
/// | `delete_table`  | Delete a table                                   |
/// | `create_field`  | Add a field to a table                           |
/// | `update_field`  | Rename a field                                   |
/// | `delete_field`  | Remove a field from a table                      |
///
/// # Filter operators for `search`
///
/// The `filter.operator` field supports: `is` (default), `is_not`,
/// `contains`, `does_not_contain`, `is_empty`, `is_not_empty`.
///
/// # Tool call examples
///
/// **Search with operator:**
/// ```json
/// {
///   "action": "search",
///   "app_token": "bascnXxx",
///   "table_id": "tblXxx",
///   "filter": {"field": "Status", "operator": "contains", "value": "Pend"}
/// }
/// ```
///
/// **Batch-update:**
/// ```json
/// {
///   "action": "batch_update",
///   "app_token": "bascnXxx",
///   "table_id": "tblXxx",
///   "records": [{"record_id": "recXxx", "fields": {"Status": "Done"}}]
/// }
/// ```
///
/// **Create table:**
/// ```json
/// {
///   "action": "create_table",
///   "app_token": "bascnXxx",
///   "table_name": "My New Table"
/// }
/// ```
pub struct LarkBitableTool {
    api: BitableApi,
}

impl LarkBitableTool {
    /// Create a new Bitable tool.
    pub fn new(config: LarkConfig) -> Self {
        Self {
            api: BitableApi::new(config),
        }
    }
}

#[async_trait]
impl Tool for LarkBitableTool {
    fn name(&self) -> &'static str {
        "lark_bitable"
    }

    fn description(&self) -> &'static str {
        "Interact with a Feishu/Lark Bitable (multi-dimensional table). \
         Supports search (with filter operators: is/is_not/contains/does_not_contain/is_empty/is_not_empty), \
         create, update, delete, batch_update, batch_delete, list_tables, list_fields, \
         create_table, delete_table, create_field, update_field, delete_field."
    }

    fn parameters(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "description": "Operation to perform.",
                    "enum": [
                        "search", "create", "update", "delete",
                        "batch_update", "batch_delete",
                        "list_tables", "list_fields",
                        "create_table", "delete_table",
                        "create_field", "update_field", "delete_field"
                    ]
                },
                "app_token": {
                    "type": "string",
                    "description": "Bitable app token (bascnXxx)"
                },
                "table_id": {
                    "type": "string",
                    "description": "Table ID (tblXxx). Required for most actions except create_table and list_tables."
                },
                "filter": {
                    "type": "object",
                    "description": "For 'search': {\"field\": \"FieldName\", \"operator\": \"is\", \"value\": \"Val\"}. operator defaults to 'is'; for is_empty/is_not_empty omit value.",
                    "properties": {
                        "field": { "type": "string" },
                        "operator": {
                            "type": "string",
                            "enum": ["is", "is_not", "contains", "does_not_contain", "is_empty", "is_not_empty"]
                        },
                        "value": {}
                    }
                },
                "records": {
                    "type": "array",
                    "description": "For 'create': [{\"FieldName\": value}]. For 'batch_update': [{\"record_id\": \"recXxx\", \"fields\": {\"FieldName\": value}}].",
                    "items": { "type": "object" }
                },
                "record_id": {
                    "type": "string",
                    "description": "For 'update'/'delete': the record ID (recXxx)"
                },
                "record_ids": {
                    "type": "array",
                    "description": "For 'batch_delete': list of record IDs to delete",
                    "items": { "type": "string" }
                },
                "fields": {
                    "type": "object",
                    "description": "For 'update': fields to update {\"FieldName\": newValue}"
                },
                "table_name": {
                    "type": "string",
                    "description": "For 'create_table': the name for the new table"
                },
                "field_name": {
                    "type": "string",
                    "description": "For 'create_field'/'update_field': the field name"
                },
                "field_type": {
                    "type": "integer",
                    "description": "For 'create_field': Feishu field type integer (1=text, 2=number, 3=single-select, etc.). Defaults to 1."
                },
                "field_id": {
                    "type": "string",
                    "description": "For 'update_field'/'delete_field': the field ID (fldXxx)"
                }
            },
            "required": ["action", "app_token"]
        }))
    }

    async fn call(&self, args: Value) -> Result<Value, SynapticError> {
        let action = args["action"]
            .as_str()
            .ok_or_else(|| SynapticError::Tool("missing 'action'".to_string()))?;
        let app_token = args["app_token"]
            .as_str()
            .ok_or_else(|| SynapticError::Tool("missing 'app_token'".to_string()))?;

        // Helper: require table_id for actions that need it.
        let require_table_id = || {
            args["table_id"]
                .as_str()
                .ok_or_else(|| SynapticError::Tool("missing 'table_id'".to_string()))
        };

        match action {
            // ── Record operations ──────────────────────────────────────────
            "search" => {
                let table_id = require_table_id()?;
                let filter = args.get("filter");
                let body = build_search_body(filter);
                let items = self.api.search_records(app_token, table_id, body).await?;
                Ok(json!({ "records": items }))
            }

            "create" => {
                let table_id = require_table_id()?;
                let raw = args["records"]
                    .as_array()
                    .ok_or_else(|| SynapticError::Tool("missing 'records' array".to_string()))?;
                let records: Vec<Value> = raw.iter().map(|r| json!({ "fields": r })).collect();
                let created = self
                    .api
                    .batch_create_records(app_token, table_id, records)
                    .await?;
                Ok(json!({ "created": created }))
            }

            "update" => {
                let table_id = require_table_id()?;
                let record_id = args["record_id"]
                    .as_str()
                    .ok_or_else(|| SynapticError::Tool("missing 'record_id'".to_string()))?;
                let fields = args
                    .get("fields")
                    .cloned()
                    .ok_or_else(|| SynapticError::Tool("missing 'fields'".to_string()))?;
                self.api
                    .update_record(app_token, table_id, record_id, fields)
                    .await?;
                Ok(json!({ "record_id": record_id, "status": "updated" }))
            }

            "delete" => {
                let table_id = require_table_id()?;
                let record_id = args["record_id"]
                    .as_str()
                    .ok_or_else(|| SynapticError::Tool("missing 'record_id'".to_string()))?;
                self.api
                    .delete_record(app_token, table_id, record_id)
                    .await?;
                Ok(json!({ "record_id": record_id, "status": "deleted" }))
            }

            "batch_update" => {
                let table_id = require_table_id()?;
                let records = args["records"]
                    .as_array()
                    .ok_or_else(|| SynapticError::Tool("missing 'records' array".to_string()))?
                    .clone();
                self.api
                    .batch_update_records(app_token, table_id, records)
                    .await?;
                Ok(json!({ "status": "updated" }))
            }

            "batch_delete" => {
                let table_id = require_table_id()?;
                let ids: Vec<String> = args["record_ids"]
                    .as_array()
                    .ok_or_else(|| SynapticError::Tool("missing 'record_ids' array".to_string()))?
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
                if ids.is_empty() {
                    return Err(SynapticError::Tool(
                        "'record_ids' must be a non-empty array".to_string(),
                    ));
                }
                self.api
                    .batch_delete_records(app_token, table_id, ids)
                    .await?;
                Ok(json!({ "status": "deleted" }))
            }

            // ── Table management ───────────────────────────────────────────
            "list_tables" => {
                let tables = self.api.list_tables(app_token).await?;
                Ok(json!({ "tables": tables }))
            }

            "create_table" => {
                let name = args["table_name"]
                    .as_str()
                    .ok_or_else(|| SynapticError::Tool("missing 'table_name'".to_string()))?;
                let table_id = self.api.create_table(app_token, name).await?;
                Ok(json!({ "table_id": table_id, "status": "created" }))
            }

            "delete_table" => {
                let table_id = require_table_id()?;
                self.api.delete_table(app_token, table_id).await?;
                Ok(json!({ "table_id": table_id, "status": "deleted" }))
            }

            // ── Field management ───────────────────────────────────────────
            "list_fields" => {
                let table_id = require_table_id()?;
                let fields = self.api.list_fields(app_token, table_id).await?;
                Ok(json!({ "fields": fields }))
            }

            "create_field" => {
                let table_id = require_table_id()?;
                let name = args["field_name"]
                    .as_str()
                    .ok_or_else(|| SynapticError::Tool("missing 'field_name'".to_string()))?;
                let field_type = args["field_type"].as_u64().unwrap_or(1) as u32;
                let field_id = self
                    .api
                    .create_field(app_token, table_id, name, field_type)
                    .await?;
                Ok(json!({ "field_id": field_id, "status": "created" }))
            }

            "update_field" => {
                let table_id = require_table_id()?;
                let field_id = args["field_id"]
                    .as_str()
                    .ok_or_else(|| SynapticError::Tool("missing 'field_id'".to_string()))?;
                let name = args["field_name"]
                    .as_str()
                    .ok_or_else(|| SynapticError::Tool("missing 'field_name'".to_string()))?;
                self.api
                    .update_field(app_token, table_id, field_id, name)
                    .await?;
                Ok(json!({ "field_id": field_id, "status": "updated" }))
            }

            "delete_field" => {
                let table_id = require_table_id()?;
                let field_id = args["field_id"]
                    .as_str()
                    .ok_or_else(|| SynapticError::Tool("missing 'field_id'".to_string()))?;
                self.api.delete_field(app_token, table_id, field_id).await?;
                Ok(json!({ "field_id": field_id, "status": "deleted" }))
            }

            other => Err(SynapticError::Tool(format!(
                "unknown action '{other}': expected search | create | update | delete | \
                 batch_update | batch_delete | list_tables | list_fields | \
                 create_table | delete_table | create_field | update_field | delete_field"
            ))),
        }
    }
}

/// Build a Bitable search request body from an optional tool-level `filter` value.
///
/// Supports operators: `is` (default), `is_not`, `contains`,
/// `does_not_contain`, `is_empty`, `is_not_empty`.
fn build_search_body(filter: Option<&Value>) -> Value {
    let f = match filter {
        None => return json!({ "page_size": 20 }),
        Some(f) => f,
    };

    let field = f["field"].as_str().unwrap_or("");
    let operator = f.get("operator").and_then(|v| v.as_str()).unwrap_or("is");

    let condition = match operator {
        "is_empty" | "is_not_empty" => json!({
            "field_name": field,
            "operator": operator
        }),
        _ => {
            let value = &f["value"];
            json!({
                "field_name": field,
                "operator": operator,
                "value": [value]
            })
        }
    };

    json!({
        "page_size": 20,
        "filter": {
            "conjunction": "and",
            "conditions": [condition]
        }
    })
}
