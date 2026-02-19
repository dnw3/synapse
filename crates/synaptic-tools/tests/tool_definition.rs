use serde_json::json;
use std::collections::HashMap;
use synaptic_core::{Tool, ToolDefinition};

struct SimpleTool;

#[async_trait::async_trait]
impl Tool for SimpleTool {
    fn name(&self) -> &'static str {
        "simple"
    }
    fn description(&self) -> &'static str {
        "A simple tool"
    }
    fn parameters(&self) -> Option<serde_json::Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "input": {"type": "string"}
            },
            "required": ["input"]
        }))
    }
    async fn call(&self, _args: serde_json::Value) -> Result<serde_json::Value, synaptic_core::SynapticError> {
        Ok(json!("ok"))
    }
}

struct NoParamsTool;

#[async_trait::async_trait]
impl Tool for NoParamsTool {
    fn name(&self) -> &'static str {
        "no_params"
    }
    fn description(&self) -> &'static str {
        "Tool with no parameters"
    }
    async fn call(&self, _args: serde_json::Value) -> Result<serde_json::Value, synaptic_core::SynapticError> {
        Ok(json!("done"))
    }
}

#[test]
fn tool_definition_extras_field() {
    let mut extras = HashMap::new();
    extras.insert("cache_control".to_string(), json!({"type": "ephemeral"}));

    let def = ToolDefinition {
        name: "search".into(),
        description: "Search the web".into(),
        parameters: json!({"type": "object", "properties": {}}),
        extras: Some(extras),
    };

    assert_eq!(def.extras.as_ref().unwrap()["cache_control"]["type"], "ephemeral");
}

#[test]
fn tool_definition_extras_none_by_default() {
    let tool = SimpleTool;
    let def = tool.as_tool_definition();
    assert!(def.extras.is_none());
}

#[test]
fn as_tool_definition_from_trait() {
    let tool = SimpleTool;
    let def = tool.as_tool_definition();
    assert_eq!(def.name, "simple");
    assert_eq!(def.description, "A simple tool");
    assert!(def.parameters["properties"]["input"].is_object());
}

#[test]
fn tool_definition_with_parameters_schema() {
    let tool = SimpleTool;
    let def = tool.as_tool_definition();
    let required = def.parameters["required"].as_array().unwrap();
    assert_eq!(required.len(), 1);
    assert_eq!(required[0], "input");
}

#[test]
fn tool_definition_default_parameters_when_none() {
    let tool = NoParamsTool;
    let def = tool.as_tool_definition();
    assert_eq!(def.parameters["type"], "object");
    assert!(def.parameters["properties"].is_object());
}

#[test]
fn tool_definition_serde_roundtrip() {
    let def = ToolDefinition {
        name: "calc".into(),
        description: "Calculator".into(),
        parameters: json!({"type": "object"}),
        extras: None,
    };
    let json = serde_json::to_string(&def).unwrap();
    let deserialized: ToolDefinition = serde_json::from_str(&json).unwrap();
    assert_eq!(def, deserialized);
}

#[test]
fn tool_definition_serde_with_extras() {
    let mut extras = HashMap::new();
    extras.insert("priority".to_string(), json!("high"));
    let def = ToolDefinition {
        name: "deploy".into(),
        description: "Deploy app".into(),
        parameters: json!({"type": "object"}),
        extras: Some(extras),
    };
    let json = serde_json::to_string(&def).unwrap();
    let deserialized: ToolDefinition = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.extras.unwrap()["priority"], "high");
}
