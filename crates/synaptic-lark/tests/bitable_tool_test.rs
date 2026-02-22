use serde_json::json;
use synaptic_core::Tool;
use synaptic_lark::{LarkBitableTool, LarkConfig};

// ── Metadata ─────────────────────────────────────────────────────────────────

#[test]
fn tool_metadata() {
    let config = LarkConfig::new("cli_test", "secret_test");
    let tool = LarkBitableTool::new(config);
    assert_eq!(tool.name(), "lark_bitable");
    assert!(!tool.description().is_empty());
    let params = tool.parameters().expect("should have parameters");
    assert!(params["properties"]["action"].is_object());
    assert!(params["properties"]["app_token"].is_object());
    assert!(params["properties"]["table_id"].is_object());
}

#[test]
fn tool_definition_required_fields() {
    let config = LarkConfig::new("cli_test", "secret_test");
    let tool = LarkBitableTool::new(config);
    let def = tool.as_tool_definition();
    let required = def.parameters["required"].as_array().unwrap();
    // table_id removed from global required (checked per-action); action + app_token always required.
    assert!(required.contains(&json!("action")));
    assert!(required.contains(&json!("app_token")));
}

#[test]
fn parameters_include_all_actions() {
    let tool = LarkBitableTool::new(LarkConfig::new("a", "b"));
    let params = tool.parameters().unwrap();
    let enum_vals = params["properties"]["action"]["enum"].as_array().unwrap();
    let actions: Vec<&str> = enum_vals.iter().filter_map(|v| v.as_str()).collect();
    // Original actions
    assert!(actions.contains(&"search"));
    assert!(actions.contains(&"create"));
    assert!(actions.contains(&"update"));
    assert!(actions.contains(&"delete"));
    assert!(actions.contains(&"list_tables"));
    assert!(actions.contains(&"list_fields"));
    // New actions
    assert!(actions.contains(&"batch_update"));
    assert!(actions.contains(&"batch_delete"));
    assert!(actions.contains(&"create_table"));
    assert!(actions.contains(&"delete_table"));
    assert!(actions.contains(&"create_field"));
    assert!(actions.contains(&"update_field"));
    assert!(actions.contains(&"delete_field"));
}

#[test]
fn filter_operator_enum_in_schema() {
    let tool = LarkBitableTool::new(LarkConfig::new("a", "b"));
    let params = tool.parameters().unwrap();
    let ops = params["properties"]["filter"]["properties"]["operator"]["enum"]
        .as_array()
        .unwrap();
    let op_strs: Vec<&str> = ops.iter().filter_map(|v| v.as_str()).collect();
    assert!(op_strs.contains(&"is"));
    assert!(op_strs.contains(&"is_not"));
    assert!(op_strs.contains(&"contains"));
    assert!(op_strs.contains(&"does_not_contain"));
    assert!(op_strs.contains(&"is_empty"));
    assert!(op_strs.contains(&"is_not_empty"));
}

// ── Validation: existing actions ──────────────────────────────────────────────

#[tokio::test]
async fn call_missing_action() {
    let config = LarkConfig::new("cli_test", "secret_test");
    let tool = LarkBitableTool::new(config);
    let err = tool
        .call(json!({
            "app_token": "bascnXxx",
            "table_id": "tblXxx"
        }))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("action"));
}

#[tokio::test]
async fn call_unknown_action() {
    let config = LarkConfig::new("cli_test", "secret_test");
    let tool = LarkBitableTool::new(config);
    let err = tool
        .call(json!({
            "action": "frobnicate",
            "app_token": "bascnXxx",
            "table_id": "tblXxx"
        }))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("unknown action"));
}

#[tokio::test]
async fn call_create_missing_records() {
    let config = LarkConfig::new("cli_test", "secret_test");
    let tool = LarkBitableTool::new(config);
    let err = tool
        .call(json!({
            "action": "create",
            "app_token": "bascnXxx",
            "table_id": "tblXxx"
        }))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("records"));
}

#[tokio::test]
async fn call_update_missing_record_id() {
    let config = LarkConfig::new("cli_test", "secret_test");
    let tool = LarkBitableTool::new(config);
    let err = tool
        .call(json!({
            "action": "update",
            "app_token": "bascnXxx",
            "table_id": "tblXxx",
            "fields": {"Status": "Done"}
        }))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("record_id"));
}

#[tokio::test]
async fn call_delete_missing_record_id() {
    let tool = LarkBitableTool::new(LarkConfig::new("a", "b"));
    let err = tool
        .call(json!({
            "action": "delete",
            "app_token": "bascnXxx",
            "table_id": "tblXxx"
        }))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("record_id"));
}

// ── Validation: new batch actions ─────────────────────────────────────────────

#[tokio::test]
async fn call_batch_update_missing_records() {
    let tool = LarkBitableTool::new(LarkConfig::new("a", "b"));
    let err = tool
        .call(json!({
            "action": "batch_update",
            "app_token": "bascnXxx",
            "table_id": "tblXxx"
        }))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("records"), "got: {err}");
}

#[tokio::test]
async fn call_batch_delete_missing_record_ids() {
    let tool = LarkBitableTool::new(LarkConfig::new("a", "b"));
    let err = tool
        .call(json!({
            "action": "batch_delete",
            "app_token": "bascnXxx",
            "table_id": "tblXxx"
        }))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("record_ids"), "got: {err}");
}

#[tokio::test]
async fn call_batch_delete_empty_record_ids() {
    let tool = LarkBitableTool::new(LarkConfig::new("a", "b"));
    let err = tool
        .call(json!({
            "action": "batch_delete",
            "app_token": "bascnXxx",
            "table_id": "tblXxx",
            "record_ids": []
        }))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("record_ids"), "got: {err}");
}

// ── Validation: table management actions ──────────────────────────────────────

#[tokio::test]
async fn call_create_table_missing_name() {
    let tool = LarkBitableTool::new(LarkConfig::new("a", "b"));
    let err = tool
        .call(json!({
            "action": "create_table",
            "app_token": "bascnXxx"
        }))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("table_name"), "got: {err}");
}

#[tokio::test]
async fn call_delete_table_missing_table_id() {
    let tool = LarkBitableTool::new(LarkConfig::new("a", "b"));
    let err = tool
        .call(json!({
            "action": "delete_table",
            "app_token": "bascnXxx"
        }))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("table_id"), "got: {err}");
}

// ── Validation: field management actions ──────────────────────────────────────

#[tokio::test]
async fn call_create_field_missing_field_name() {
    let tool = LarkBitableTool::new(LarkConfig::new("a", "b"));
    let err = tool
        .call(json!({
            "action": "create_field",
            "app_token": "bascnXxx",
            "table_id": "tblXxx"
        }))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("field_name"), "got: {err}");
}

#[tokio::test]
async fn call_update_field_missing_field_id() {
    let tool = LarkBitableTool::new(LarkConfig::new("a", "b"));
    let err = tool
        .call(json!({
            "action": "update_field",
            "app_token": "bascnXxx",
            "table_id": "tblXxx",
            "field_name": "NewName"
        }))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("field_id"), "got: {err}");
}

#[tokio::test]
async fn call_update_field_missing_field_name() {
    let tool = LarkBitableTool::new(LarkConfig::new("a", "b"));
    let err = tool
        .call(json!({
            "action": "update_field",
            "app_token": "bascnXxx",
            "table_id": "tblXxx",
            "field_id": "fldXxx"
        }))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("field_name"), "got: {err}");
}

#[tokio::test]
async fn call_delete_field_missing_field_id() {
    let tool = LarkBitableTool::new(LarkConfig::new("a", "b"));
    let err = tool
        .call(json!({
            "action": "delete_field",
            "app_token": "bascnXxx",
            "table_id": "tblXxx"
        }))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("field_id"), "got: {err}");
}

// ── Accepted-action smoke tests (validation passes, network fails) ────────────

#[tokio::test]
async fn call_list_tables_accepted() {
    let tool = LarkBitableTool::new(LarkConfig::new("a", "b"));
    let result = tool
        .call(json!({
            "action": "list_tables",
            "app_token": "bascnXxx"
        }))
        .await;
    let err_str = result.unwrap_err().to_string();
    assert!(!err_str.contains("unknown action"), "got: {err_str}");
}

#[tokio::test]
async fn call_list_fields_accepted() {
    let tool = LarkBitableTool::new(LarkConfig::new("a", "b"));
    let result = tool
        .call(json!({
            "action": "list_fields",
            "app_token": "bascnXxx",
            "table_id": "tblXxx"
        }))
        .await;
    let err_str = result.unwrap_err().to_string();
    assert!(!err_str.contains("unknown action"), "got: {err_str}");
}

#[tokio::test]
async fn call_batch_update_accepted() {
    let tool = LarkBitableTool::new(LarkConfig::new("a", "b"));
    let result = tool
        .call(json!({
            "action": "batch_update",
            "app_token": "bascnXxx",
            "table_id": "tblXxx",
            "records": [{"record_id": "recXxx", "fields": {"Status": "Done"}}]
        }))
        .await;
    let err_str = result.unwrap_err().to_string();
    assert!(!err_str.contains("unknown action"), "got: {err_str}");
}

#[tokio::test]
async fn call_batch_delete_accepted() {
    let tool = LarkBitableTool::new(LarkConfig::new("a", "b"));
    let result = tool
        .call(json!({
            "action": "batch_delete",
            "app_token": "bascnXxx",
            "table_id": "tblXxx",
            "record_ids": ["recXxx"]
        }))
        .await;
    let err_str = result.unwrap_err().to_string();
    assert!(!err_str.contains("unknown action"), "got: {err_str}");
}

#[tokio::test]
async fn call_create_table_accepted() {
    let tool = LarkBitableTool::new(LarkConfig::new("a", "b"));
    let result = tool
        .call(json!({
            "action": "create_table",
            "app_token": "bascnXxx",
            "table_name": "My Table"
        }))
        .await;
    let err_str = result.unwrap_err().to_string();
    assert!(!err_str.contains("unknown action"), "got: {err_str}");
}

#[tokio::test]
async fn call_delete_table_accepted() {
    let tool = LarkBitableTool::new(LarkConfig::new("a", "b"));
    let result = tool
        .call(json!({
            "action": "delete_table",
            "app_token": "bascnXxx",
            "table_id": "tblXxx"
        }))
        .await;
    let err_str = result.unwrap_err().to_string();
    assert!(!err_str.contains("unknown action"), "got: {err_str}");
}

#[tokio::test]
async fn call_create_field_accepted() {
    let tool = LarkBitableTool::new(LarkConfig::new("a", "b"));
    let result = tool
        .call(json!({
            "action": "create_field",
            "app_token": "bascnXxx",
            "table_id": "tblXxx",
            "field_name": "My Field"
        }))
        .await;
    let err_str = result.unwrap_err().to_string();
    assert!(!err_str.contains("unknown action"), "got: {err_str}");
}

#[tokio::test]
async fn call_update_field_accepted() {
    let tool = LarkBitableTool::new(LarkConfig::new("a", "b"));
    let result = tool
        .call(json!({
            "action": "update_field",
            "app_token": "bascnXxx",
            "table_id": "tblXxx",
            "field_id": "fldXxx",
            "field_name": "Renamed"
        }))
        .await;
    let err_str = result.unwrap_err().to_string();
    assert!(!err_str.contains("unknown action"), "got: {err_str}");
}

#[tokio::test]
async fn call_delete_field_accepted() {
    let tool = LarkBitableTool::new(LarkConfig::new("a", "b"));
    let result = tool
        .call(json!({
            "action": "delete_field",
            "app_token": "bascnXxx",
            "table_id": "tblXxx",
            "field_id": "fldXxx"
        }))
        .await;
    let err_str = result.unwrap_err().to_string();
    assert!(!err_str.contains("unknown action"), "got: {err_str}");
}

// ── Integration tests (skipped without credentials) ──────────────────────────

#[tokio::test]
#[ignore = "requires LARK_APP_ID and LARK_APP_SECRET"]
async fn integration_search_records() {
    let app_id = std::env::var("LARK_APP_ID").unwrap();
    let app_secret = std::env::var("LARK_APP_SECRET").unwrap();
    let app_token = std::env::var("LARK_BITABLE_APP_TOKEN").unwrap();
    let table_id = std::env::var("LARK_BITABLE_TABLE_ID").unwrap();

    let config = LarkConfig::new(app_id, app_secret);
    let tool = LarkBitableTool::new(config);
    let result = tool
        .call(json!({
            "action": "search",
            "app_token": app_token,
            "table_id": table_id
        }))
        .await
        .expect("search should succeed");
    assert!(result["records"].is_array());
}
