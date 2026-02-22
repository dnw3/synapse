use serde_json::json;
use synaptic_core::Tool;
use synaptic_lark::{LarkConfig, LarkMessageTool};

// ── Metadata ─────────────────────────────────────────────────────────────────

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
    assert!(params["properties"]["message_id"].is_object());
    assert!(params["properties"]["action"].is_object());
}

#[test]
fn tool_definition() {
    let config = LarkConfig::new("cli_test", "secret_test");
    let tool = LarkMessageTool::new(config);
    let def = tool.as_tool_definition();
    assert_eq!(def.name, "lark_send_message");
    // Only "action" is globally required; per-action fields are validated in call()
    let required = def.parameters["required"].as_array().unwrap();
    assert!(required.contains(&json!("action")));
}

#[test]
fn action_enum_includes_all_ops() {
    let tool = LarkMessageTool::new(LarkConfig::new("a", "b"));
    let params = tool.parameters().unwrap();
    let enum_vals = params["properties"]["action"]["enum"].as_array().unwrap();
    let actions: Vec<&str> = enum_vals.iter().filter_map(|v| v.as_str()).collect();
    assert!(actions.contains(&"send"));
    assert!(actions.contains(&"update"));
    assert!(actions.contains(&"delete"));
}

// ── Validation: send ──────────────────────────────────────────────────────────

#[tokio::test]
async fn call_send_missing_content() {
    let config = LarkConfig::new("cli_test", "secret_test");
    let tool = LarkMessageTool::new(config);
    let err = tool
        .call(json!({
            "action": "send",
            "receive_id_type": "chat_id",
            "receive_id": "oc_xxx",
            "msg_type": "text"
            // missing content
        }))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("content"), "got: {err}");
}

#[tokio::test]
async fn call_send_invalid_post_json() {
    let config = LarkConfig::new("cli_test", "secret_test");
    let tool = LarkMessageTool::new(config);
    let err = tool
        .call(json!({
            "action": "send",
            "receive_id_type": "chat_id",
            "receive_id": "oc_xxx",
            "msg_type": "post",
            "content": "not valid json {"
        }))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("valid JSON"), "got: {err}");
}

/// Backward-compatible: no `action` field defaults to "send".
#[tokio::test]
async fn call_without_action_defaults_to_send() {
    let tool = LarkMessageTool::new(LarkConfig::new("a", "b"));
    // Validation passes (missing receive_id_type detected at call validation)
    let err = tool
        .call(json!({
            "receive_id_type": "chat_id",
            "receive_id": "oc_xxx",
            "msg_type": "text",
            "content": "hello"
        }))
        .await
        .unwrap_err();
    // Should fail with network/auth error, NOT "unknown action"
    assert!(!err.to_string().contains("unknown action"), "got: {err}");
}

// ── Validation: update ────────────────────────────────────────────────────────

#[tokio::test]
async fn call_update_missing_message_id() {
    let tool = LarkMessageTool::new(LarkConfig::new("a", "b"));
    let err = tool
        .call(json!({
            "action": "update",
            "msg_type": "text",
            "content": "new content"
        }))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("message_id"), "got: {err}");
}

#[tokio::test]
async fn call_update_missing_msg_type() {
    let tool = LarkMessageTool::new(LarkConfig::new("a", "b"));
    let err = tool
        .call(json!({
            "action": "update",
            "message_id": "om_xxx",
            "content": "new content"
        }))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("msg_type"), "got: {err}");
}

#[tokio::test]
async fn call_update_missing_content() {
    let tool = LarkMessageTool::new(LarkConfig::new("a", "b"));
    let err = tool
        .call(json!({
            "action": "update",
            "message_id": "om_xxx",
            "msg_type": "text"
        }))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("content"), "got: {err}");
}

#[tokio::test]
async fn call_update_invalid_interactive_json() {
    let tool = LarkMessageTool::new(LarkConfig::new("a", "b"));
    let err = tool
        .call(json!({
            "action": "update",
            "message_id": "om_xxx",
            "msg_type": "interactive",
            "content": "not json {"
        }))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("valid JSON"), "got: {err}");
}

// ── Validation: delete ────────────────────────────────────────────────────────

#[tokio::test]
async fn call_delete_missing_message_id() {
    let tool = LarkMessageTool::new(LarkConfig::new("a", "b"));
    let err = tool
        .call(json!({
            "action": "delete"
        }))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("message_id"), "got: {err}");
}

// ── Unknown action ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn call_unknown_action() {
    let tool = LarkMessageTool::new(LarkConfig::new("a", "b"));
    let err = tool
        .call(json!({ "action": "recall_all" }))
        .await
        .unwrap_err();
    assert!(err.to_string().contains("unknown action"), "got: {err}");
}

// ── Accepted-action smoke tests ───────────────────────────────────────────────

#[tokio::test]
async fn call_delete_accepted() {
    let tool = LarkMessageTool::new(LarkConfig::new("a", "b"));
    let result = tool
        .call(json!({
            "action": "delete",
            "message_id": "om_xxx"
        }))
        .await;
    // Should fail with network/auth, NOT "unknown action" or validation error
    let err_str = result.unwrap_err().to_string();
    assert!(!err_str.contains("unknown action"), "got: {err_str}");
    assert!(!err_str.contains("message_id"), "got: {err_str}");
}

// ── Integration tests (skipped without credentials) ──────────────────────────

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
            "action": "send",
            "receive_id_type": "chat_id",
            "receive_id": chat_id,
            "msg_type": "text",
            "content": "Synaptic integration test message"
        }))
        .await
        .expect("send should succeed");
    assert!(!result["message_id"].as_str().unwrap().is_empty());
    assert_eq!(result["status"], "sent");
}
