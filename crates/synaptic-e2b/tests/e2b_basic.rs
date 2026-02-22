use synaptic_core::Tool;
use synaptic_e2b::{E2BConfig, E2BSandboxTool};

#[test]
fn test_config_defaults() {
    let config = E2BConfig::new("test-key");
    assert_eq!(config.template, "base");
    assert_eq!(config.timeout_secs, 30);
}

#[test]
fn test_config_builder() {
    let config = E2BConfig::new("key")
        .with_template("python")
        .with_timeout(60);
    assert_eq!(config.template, "python");
    assert_eq!(config.timeout_secs, 60);
}

#[test]
fn test_tool_name() {
    let tool = E2BSandboxTool::new(E2BConfig::new("key"));
    assert_eq!(tool.name(), "e2b_code_executor");
}

#[test]
fn test_tool_parameters() {
    let tool = E2BSandboxTool::new(E2BConfig::new("key"));
    let params = tool.parameters().unwrap();
    assert_eq!(params["type"], "object");
    assert!(params["properties"]["code"].is_object());
    assert!(params["properties"]["language"].is_object());
}

#[tokio::test]
#[ignore]
async fn test_execute_python_integration() {
    let api_key = std::env::var("E2B_API_KEY").unwrap();
    let tool = E2BSandboxTool::new(E2BConfig::new(api_key));
    let result = tool
        .call(serde_json::json!({
            "code": "print('hello from e2b')",
            "language": "python"
        }))
        .await
        .unwrap();
    assert!(result["stdout"].as_str().unwrap_or("").contains("hello"));
}
