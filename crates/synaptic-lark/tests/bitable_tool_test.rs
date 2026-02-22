use serde_json::json;
use synaptic_core::Tool;
use synaptic_lark::{LarkBitableTool, LarkConfig};

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
    assert!(required.contains(&json!("action")));
    assert!(required.contains(&json!("app_token")));
    assert!(required.contains(&json!("table_id")));
}

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
            "action": "delete",
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
            // missing records
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
            // missing record_id
        }))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("record_id"));
}

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
