use serde_json::json;
use synaptic_core::Tool;
use synaptic_lark::{LarkConfig, LarkMessageTool};

#[test]
fn tool_metadata() {
    let config = LarkConfig::new("cli_test", "secret_test");
    let tool = LarkMessageTool::new(config);
    assert_eq!(tool.name(), "lark_send_message");
    assert!(!tool.description().is_empty());
    let params = tool.parameters().expect("should have parameters");
    assert!(params["properties"]["receive_id_type"].is_object());
    assert!(params["properties"]["receive_id"].is_object());
    assert!(params["properties"]["msg_type"].is_object());
    assert!(params["properties"]["content"].is_object());
}

#[test]
fn tool_definition() {
    let config = LarkConfig::new("cli_test", "secret_test");
    let tool = LarkMessageTool::new(config);
    let def = tool.as_tool_definition();
    assert_eq!(def.name, "lark_send_message");
    let required = def.parameters["required"].as_array().unwrap();
    assert!(required.contains(&json!("receive_id_type")));
    assert!(required.contains(&json!("receive_id")));
    assert!(required.contains(&json!("msg_type")));
    assert!(required.contains(&json!("content")));
}

#[tokio::test]
async fn call_missing_required_field() {
    let config = LarkConfig::new("cli_test", "secret_test");
    let tool = LarkMessageTool::new(config);
    let err = tool
        .call(json!({
            "receive_id_type": "chat_id",
            "receive_id": "oc_xxx",
            "msg_type": "text"
            // missing content
        }))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("content"));
}

#[tokio::test]
async fn call_invalid_post_json() {
    let config = LarkConfig::new("cli_test", "secret_test");
    let tool = LarkMessageTool::new(config);
    // post requires content to be valid JSON string
    let err = tool
        .call(json!({
            "receive_id_type": "chat_id",
            "receive_id": "oc_xxx",
            "msg_type": "post",
            "content": "not valid json {"
        }))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("valid JSON"));
}

#[tokio::test]
#[ignore = "requires LARK_APP_ID and LARK_APP_SECRET"]
async fn integration_send_message() {
    let app_id = std::env::var("LARK_APP_ID").unwrap();
    let app_secret = std::env::var("LARK_APP_SECRET").unwrap();
    let chat_id = std::env::var("LARK_CHAT_ID").unwrap();

    let config = LarkConfig::new(app_id, app_secret);
    let tool = LarkMessageTool::new(config);
    let result = tool
        .call(json!({
            "receive_id_type": "chat_id",
            "receive_id": chat_id,
            "msg_type": "text",
            "content": "Synaptic integration test message"
        }))
        .await
        .expect("send should succeed");
    assert!(result["message_id"].as_str().unwrap().len() > 0);
}
