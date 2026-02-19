//! Edge-case tests for the `#[tool]` macro: zero params, many typed params,
//! `#[args]` passthrough, description extraction, and schema validation.

use serde_json::{json, Value};
use synaptic_core::SynapticError;
use synaptic_macros::tool;

// ---------------------------------------------------------------------------
// Tool with zero parameters
// ---------------------------------------------------------------------------

/// A tool that takes no parameters.
#[tool(name = "noop")]
async fn noop() -> Result<Value, SynapticError> {
    Ok(json!("done"))
}

#[tokio::test]
async fn noop_tool_no_parameters() {
    let tool = noop();
    assert_eq!(tool.name(), "noop");
    // Zero schema params ⇒ parameters() returns None
    assert!(tool.parameters().is_none());
    let result = tool.call(json!({})).await.unwrap();
    assert_eq!(result, json!("done"));
}

#[tokio::test]
async fn noop_tool_has_description_from_doc() {
    let tool = noop();
    assert_eq!(tool.description(), "A tool that takes no parameters.");
}

#[test]
fn noop_tool_definition_name_matches() {
    let tool = noop();
    let def = tool.as_tool_definition();
    assert_eq!(def.name, "noop");
    assert_eq!(def.description, "A tool that takes no parameters.");
}

// ---------------------------------------------------------------------------
// Tool with many typed parameters
// ---------------------------------------------------------------------------

/// A tool with many parameter types.
#[tool(name = "typed")]
async fn typed_params(
    /// A string
    text: String,
    /// A number
    count: i64,
    /// A float
    ratio: f64,
    /// A boolean
    flag: bool,
) -> Result<Value, SynapticError> {
    Ok(json!({
        "text": text,
        "count": count,
        "ratio": ratio,
        "flag": flag,
    }))
}

#[tokio::test]
async fn typed_params_all_types() {
    let tool = typed_params();
    let result = tool
        .call(json!({
            "text": "hello",
            "count": 42,
            "ratio": 3.14,
            "flag": true,
        }))
        .await
        .unwrap();
    assert_eq!(result["text"], "hello");
    assert_eq!(result["count"], 42);
    assert_eq!(result["ratio"], 3.14);
    assert_eq!(result["flag"], true);
}

#[tokio::test]
async fn typed_params_missing_required_errors() {
    let tool = typed_params();
    // Only provide "text" — missing count, ratio, flag
    let result = tool.call(json!({"text": "hello"})).await;
    assert!(result.is_err());
}

#[tokio::test]
async fn typed_params_schema_has_all_required() {
    let tool = typed_params();
    let params = tool.parameters().unwrap();
    let required = params["required"].as_array().unwrap();
    assert!(required.contains(&json!("text")));
    assert!(required.contains(&json!("count")));
    assert!(required.contains(&json!("ratio")));
    assert!(required.contains(&json!("flag")));
    assert_eq!(required.len(), 4);
}

#[tokio::test]
async fn typed_params_schema_property_types() {
    let tool = typed_params();
    let params = tool.parameters().unwrap();
    let props = params["properties"].as_object().unwrap();
    assert_eq!(props["text"]["type"], "string");
    assert_eq!(props["count"]["type"], "integer");
    assert_eq!(props["ratio"]["type"], "number");
    assert_eq!(props["flag"]["type"], "boolean");
}

#[tokio::test]
async fn typed_params_descriptions_in_schema() {
    let tool = typed_params();
    let params = tool.parameters().unwrap();
    let props = &params["properties"];
    assert_eq!(props["text"]["description"], "A string");
    assert_eq!(props["count"]["description"], "A number");
    assert_eq!(props["ratio"]["description"], "A float");
    assert_eq!(props["flag"]["description"], "A boolean");
}

// ---------------------------------------------------------------------------
// Tool with #[args] passthrough (identity)
// ---------------------------------------------------------------------------

/// Returns the input unchanged.
#[tool(name = "identity")]
async fn identity(#[args] args: Value) -> Result<Value, SynapticError> {
    Ok(args)
}

#[tokio::test]
async fn identity_passthrough() {
    let tool = identity();
    let input = json!({"any": "data", "nested": [1, 2, 3]});
    let result = tool.call(input.clone()).await.unwrap();
    assert_eq!(result, input);
}

#[tokio::test]
async fn identity_empty_object() {
    let tool = identity();
    let result = tool.call(json!({})).await.unwrap();
    assert_eq!(result, json!({}));
}

#[tokio::test]
async fn identity_no_parameters_schema() {
    // #[args] tools have no JSON schema (parameters is None)
    let tool = identity();
    assert!(tool.parameters().is_none());
}

// ---------------------------------------------------------------------------
// Tool returning non-Value type
// ---------------------------------------------------------------------------

/// Concatenate two strings.
#[tool(name = "concat")]
async fn concat_strings(
    /// First string
    a: String,
    /// Second string
    b: String,
) -> Result<String, SynapticError> {
    Ok(format!("{}{}", a, b))
}

#[tokio::test]
async fn concat_returns_json_string() {
    let tool = concat_strings();
    let result = tool.call(json!({"a": "hello", "b": " world"})).await.unwrap();
    assert_eq!(result, json!("hello world"));
}

// ---------------------------------------------------------------------------
// Tool with Vec parameter
// ---------------------------------------------------------------------------

/// Sum a list of integers.
#[tool(name = "sum")]
async fn sum_list(
    /// The numbers to sum
    values: Vec<i64>,
) -> Result<i64, SynapticError> {
    Ok(values.iter().sum())
}

#[tokio::test]
async fn vec_param_schema_is_array() {
    let tool = sum_list();
    let params = tool.parameters().unwrap();
    let props = params["properties"].as_object().unwrap();
    assert_eq!(props["values"]["type"], "array");
    assert_eq!(props["values"]["items"]["type"], "integer");
}

#[tokio::test]
async fn vec_param_call_works() {
    let tool = sum_list();
    let result = tool.call(json!({"values": [1, 2, 3, 4]})).await.unwrap();
    assert_eq!(result, json!(10));
}

// ---------------------------------------------------------------------------
// Tool with Option + default combined
// ---------------------------------------------------------------------------

/// Format a greeting.
#[tool]
async fn fancy_greet(
    /// The name to greet
    name: String,
    /// Optional title
    title: Option<String>,
    /// Exclamation count
    #[default = 1]
    exclaim: i64,
) -> Result<String, SynapticError> {
    let prefix = title.map(|t| format!("{} ", t)).unwrap_or_default();
    let bangs = "!".repeat(exclaim as usize);
    Ok(format!("Hello, {}{}{}",  prefix, name, bangs))
}

#[tokio::test]
async fn option_and_default_together() {
    let tool = fancy_greet();
    // Only required param
    let result = tool.call(json!({"name": "Alice"})).await.unwrap();
    assert_eq!(result, json!("Hello, Alice!"));
}

#[tokio::test]
async fn option_and_default_all_provided() {
    let tool = fancy_greet();
    let result = tool
        .call(json!({"name": "Alice", "title": "Dr.", "exclaim": 3}))
        .await
        .unwrap();
    assert_eq!(result, json!("Hello, Dr. Alice!!!"));
}

#[tokio::test]
async fn option_and_default_required_list() {
    let tool = fancy_greet();
    let params = tool.parameters().unwrap();
    let required = params["required"].as_array().unwrap();
    // Only "name" should be required
    assert_eq!(required.len(), 1);
    assert!(required.contains(&json!("name")));
}
