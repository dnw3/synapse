use synaptic_core::Tool;
use synaptic_e2b::{E2BConfig, E2BSandboxTool};

#[test]
fn test_tool_description_contains_sandbox() {
    let tool = E2BSandboxTool::new(E2BConfig::new("key"));
    assert!(tool.description().contains("sandbox") || tool.description().contains("E2B"));
}

#[test]
fn test_api_key_stored() {
    let config = E2BConfig::new("e2b-secret-key");
    assert_eq!(config.api_key, "e2b-secret-key");
}
