//! Integration tests for the `#[tool]` macro.

use serde_json::{json, Value};
use synaptic_core::SynapticError;
use synaptic_macros::tool;

// ---------------------------------------------------------------------------
// Basic tool: no defaults, no options, no inject
// ---------------------------------------------------------------------------

/// Search the web for information.
#[tool]
async fn search(query: String) -> Result<String, SynapticError> {
    Ok(format!("Results for: {}", query))
}

#[tokio::test]
async fn test_basic_tool_name() {
    let t = search();
    assert_eq!(t.name(), "search");
}

#[tokio::test]
async fn test_basic_tool_description() {
    let t = search();
    assert_eq!(t.description(), "Search the web for information.");
}

#[tokio::test]
async fn test_basic_tool_parameters() {
    let t = search();
    let params = t.parameters().unwrap();
    let props = params.get("properties").unwrap();
    assert!(props.get("query").is_some());
    let required = params.get("required").unwrap().as_array().unwrap();
    assert!(required.contains(&json!("query")));
}

#[tokio::test]
async fn test_basic_tool_call() {
    let t = search();
    let result = t.call(json!({"query": "rust lang"})).await.unwrap();
    assert_eq!(result, json!("Results for: rust lang"));
}

#[tokio::test]
async fn test_basic_tool_missing_param() {
    let t = search();
    let result = t.call(json!({})).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("missing required parameter: query"));
}

// ---------------------------------------------------------------------------
// Tool with default value
// ---------------------------------------------------------------------------

/// Multiply two numbers.
#[tool]
async fn multiply(
    /// The first operand
    a: f64,
    /// The second operand
    b: f64,
    /// Scale factor
    #[default = 1.0]
    scale: f64,
) -> Result<f64, SynapticError> {
    Ok(a * b * scale)
}

#[tokio::test]
async fn test_default_value_used() {
    let t = multiply();
    let result = t.call(json!({"a": 3.0, "b": 4.0})).await.unwrap();
    assert_eq!(result, json!(12.0));
}

#[tokio::test]
async fn test_default_value_overridden() {
    let t = multiply();
    let result = t
        .call(json!({"a": 3.0, "b": 4.0, "scale": 2.0}))
        .await
        .unwrap();
    assert_eq!(result, json!(24.0));
}

#[tokio::test]
async fn test_param_descriptions_in_schema() {
    let t = multiply();
    let params = t.parameters().unwrap();
    let props = params.get("properties").unwrap();
    let a_schema = props.get("a").unwrap();
    assert_eq!(
        a_schema.get("description").unwrap().as_str().unwrap(),
        "The first operand"
    );
}

#[tokio::test]
async fn test_default_not_in_required() {
    let t = multiply();
    let params = t.parameters().unwrap();
    let required = params.get("required").unwrap().as_array().unwrap();
    assert!(required.contains(&json!("a")));
    assert!(required.contains(&json!("b")));
    assert!(!required.contains(&json!("scale")));
}

// ---------------------------------------------------------------------------
// Tool with Option parameter
// ---------------------------------------------------------------------------

/// Greet someone.
#[tool]
async fn greet(name: String, title: Option<String>) -> Result<String, SynapticError> {
    match title {
        Some(t) => Ok(format!("Hello, {} {}!", t, name)),
        None => Ok(format!("Hello, {}!", name)),
    }
}

#[tokio::test]
async fn test_option_param_provided() {
    let t = greet();
    let result = t
        .call(json!({"name": "Alice", "title": "Dr."}))
        .await
        .unwrap();
    assert_eq!(result, json!("Hello, Dr. Alice!"));
}

#[tokio::test]
async fn test_option_param_absent() {
    let t = greet();
    let result = t.call(json!({"name": "Bob"})).await.unwrap();
    assert_eq!(result, json!("Hello, Bob!"));
}

#[tokio::test]
async fn test_option_not_in_required() {
    let t = greet();
    let params = t.parameters().unwrap();
    let required = params.get("required").unwrap().as_array().unwrap();
    assert!(required.contains(&json!("name")));
    assert!(!required.contains(&json!("title")));
}

// ---------------------------------------------------------------------------
// Tool with custom name
// ---------------------------------------------------------------------------

/// Does calculations.
#[tool(name = "calculator")]
async fn calc(expression: String) -> Result<String, SynapticError> {
    Ok(format!("Calculated: {}", expression))
}

#[tokio::test]
async fn test_custom_name() {
    let t = calc();
    assert_eq!(t.name(), "calculator");
}

// ---------------------------------------------------------------------------
// Tool with Vec parameter
// ---------------------------------------------------------------------------

/// Sum a list of numbers.
#[tool]
async fn sum_numbers(numbers: Vec<f64>) -> Result<f64, SynapticError> {
    Ok(numbers.iter().sum())
}

#[tokio::test]
async fn test_vec_param() {
    let t = sum_numbers();
    let result = t.call(json!({"numbers": [1.0, 2.0, 3.0]})).await.unwrap();
    assert_eq!(result, json!(6.0));
}

#[tokio::test]
async fn test_vec_schema() {
    let t = sum_numbers();
    let params = t.parameters().unwrap();
    let props = params.get("properties").unwrap();
    let numbers_schema = props.get("numbers").unwrap();
    assert_eq!(
        numbers_schema.get("type").unwrap().as_str().unwrap(),
        "array"
    );
}

// ---------------------------------------------------------------------------
// Tool with bool parameter
// ---------------------------------------------------------------------------

/// Format a message.
#[tool]
async fn format_msg(text: String, uppercase: bool) -> Result<String, SynapticError> {
    if uppercase {
        Ok(text.to_uppercase())
    } else {
        Ok(text)
    }
}

#[tokio::test]
async fn test_bool_param() {
    let t = format_msg();
    let result = t
        .call(json!({"text": "hello", "uppercase": true}))
        .await
        .unwrap();
    assert_eq!(result, json!("HELLO"));
}

// ---------------------------------------------------------------------------
// Tool with integer params
// ---------------------------------------------------------------------------

/// Add integers.
#[tool]
async fn add_ints(a: i64, b: i64) -> Result<i64, SynapticError> {
    Ok(a + b)
}

#[tokio::test]
async fn test_integer_params() {
    let t = add_ints();
    let result = t.call(json!({"a": 10, "b": 20})).await.unwrap();
    assert_eq!(result, json!(30));
}

#[tokio::test]
async fn test_integer_schema() {
    let t = add_ints();
    let params = t.parameters().unwrap();
    let props = params.get("properties").unwrap();
    assert_eq!(
        props
            .get("a")
            .unwrap()
            .get("type")
            .unwrap()
            .as_str()
            .unwrap(),
        "integer"
    );
}

// ---------------------------------------------------------------------------
// as_tool_definition
// ---------------------------------------------------------------------------

#[tokio::test]
async fn test_as_tool_definition() {
    let t = search();
    let def = t.as_tool_definition();
    assert_eq!(def.name, "search");
    assert_eq!(def.description, "Search the web for information.");
    assert!(def.parameters.get("properties").is_some());
}

// ---------------------------------------------------------------------------
// Tool with #[field] — stateful tool with struct fields
// ---------------------------------------------------------------------------

use std::sync::Arc;

/// A database lookup tool.
#[tool]
async fn db_lookup(
    #[field] connection: Arc<String>,
    /// The table to query
    table: String,
) -> Result<String, SynapticError> {
    Ok(format!("Querying {} on {}", table, connection))
}

#[tokio::test]
async fn test_field_factory_takes_param() {
    let conn = Arc::new("postgres://localhost".to_string());
    let t = db_lookup(conn);
    assert_eq!(t.name(), "db_lookup");
    assert_eq!(t.description(), "A database lookup tool.");
}

#[tokio::test]
async fn test_field_excluded_from_schema() {
    let conn = Arc::new("postgres://localhost".to_string());
    let t = db_lookup(conn);
    let params = t.parameters().unwrap();
    let props = params.get("properties").unwrap();
    // "connection" should NOT be in the schema
    assert!(props.get("connection").is_none());
    // "table" should be in the schema
    assert!(props.get("table").is_some());
}

#[tokio::test]
async fn test_field_call_uses_struct_field() {
    let conn = Arc::new("my_db".to_string());
    let t = db_lookup(conn);
    let result = t.call(json!({"table": "users"})).await.unwrap();
    assert_eq!(result, json!("Querying users on my_db"));
}

#[tokio::test]
async fn test_field_required_list() {
    let conn = Arc::new("db".to_string());
    let t = db_lookup(conn);
    let params = t.parameters().unwrap();
    let required = params.get("required").unwrap().as_array().unwrap();
    assert!(required.contains(&json!("table")));
    assert!(!required.contains(&json!("connection")));
}

// ---------------------------------------------------------------------------
// Tool with multiple #[field] params + mixed with defaults
// ---------------------------------------------------------------------------

/// A multi-field tool.
#[tool]
async fn multi_field(
    #[field] prefix: String,
    #[field] suffix: String,
    /// The input text
    text: String,
    /// Repeat count
    #[default = 1]
    repeat: i64,
) -> Result<String, SynapticError> {
    let inner = text.repeat(repeat as usize);
    Ok(format!("{}{}{}", prefix, inner, suffix))
}

#[tokio::test]
async fn test_multiple_fields() {
    let t = multi_field("<<".to_string(), ">>".to_string());
    let result = t.call(json!({"text": "hi", "repeat": 2})).await.unwrap();
    assert_eq!(result, json!("<<hihi>>"));
}

#[tokio::test]
async fn test_multiple_fields_schema_excludes_fields() {
    let t = multi_field("a".to_string(), "b".to_string());
    let params = t.parameters().unwrap();
    let props = params.get("properties").unwrap();
    assert!(props.get("prefix").is_none());
    assert!(props.get("suffix").is_none());
    assert!(props.get("text").is_some());
    assert!(props.get("repeat").is_some());
}

#[tokio::test]
async fn test_multiple_fields_default_works() {
    let t = multi_field("[".to_string(), "]".to_string());
    let result = t.call(json!({"text": "x"})).await.unwrap();
    assert_eq!(result, json!("[x]"));
}

// ---------------------------------------------------------------------------
// Tool with #[args] — raw Value passthrough
// ---------------------------------------------------------------------------

/// Echo input back.
#[tool(name = "echo")]
async fn echo(#[args] args: Value) -> Result<Value, SynapticError> {
    Ok(json!({"echo": args}))
}

#[tokio::test]
async fn test_args_receives_raw_value() {
    let t = echo();
    let input = json!({"foo": "bar", "n": 42});
    let result = t.call(input.clone()).await.unwrap();
    assert_eq!(result, json!({"echo": {"foo": "bar", "n": 42}}));
}

#[tokio::test]
async fn test_args_no_schema() {
    let t = echo();
    assert!(t.parameters().is_none());
}

#[tokio::test]
async fn test_args_name_and_description() {
    let t = echo();
    assert_eq!(t.name(), "echo");
    assert_eq!(t.description(), "Echo input back.");
}

// ---------------------------------------------------------------------------
// Tool with #[args] + #[field] mixed
// ---------------------------------------------------------------------------

/// Echo with prefix.
#[tool]
async fn echo_with_prefix(
    #[field] prefix: String,
    #[args] args: Value,
) -> Result<Value, SynapticError> {
    Ok(json!({"prefix": prefix, "data": args}))
}

#[tokio::test]
async fn test_args_with_field() {
    let t = echo_with_prefix(">>".to_string());
    let result = t.call(json!({"x": 1})).await.unwrap();
    assert_eq!(result, json!({"prefix": ">>", "data": {"x": 1}}));
}

#[tokio::test]
async fn test_args_with_field_no_schema() {
    let t = echo_with_prefix("p".to_string());
    // parameters should be None since the only non-field param is #[args]
    assert!(t.parameters().is_none());
}
