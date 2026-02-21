use serde_json::json;
use synaptic_core::Tool;
use synaptic_tavily::{TavilyConfig, TavilySearchTool};

#[test]
fn tool_metadata() {
    let config = TavilyConfig::new("test-key");
    let tool = TavilySearchTool::new(config);
    assert_eq!(tool.name(), "tavily_search");
    assert!(!tool.description().is_empty());
    assert!(tool.parameters().is_some());
}

#[test]
fn tool_definition() {
    let config = TavilyConfig::new("test-key");
    let tool = TavilySearchTool::new(config);
    let def = tool.as_tool_definition();
    assert_eq!(def.name, "tavily_search");
    assert!(!def.description.is_empty());

    // Parameters should include a "query" property
    let props = def.parameters.get("properties").unwrap();
    assert!(props.get("query").is_some());

    let required = def.parameters.get("required").unwrap().as_array().unwrap();
    assert!(required.contains(&json!("query")));
}

#[test]
fn config_defaults() {
    let config = TavilyConfig::new("key");
    assert_eq!(config.max_results, 5);
    assert_eq!(config.search_depth, "basic");
    assert!(config.include_answer);
    assert_eq!(config.base_url, "https://api.tavily.com");
}

#[test]
fn config_builder() {
    let config = TavilyConfig::new("key")
        .with_max_results(10)
        .with_search_depth("advanced")
        .with_include_answer(false)
        .with_base_url("https://custom.api.com");
    assert_eq!(config.max_results, 10);
    assert_eq!(config.search_depth, "advanced");
    assert!(!config.include_answer);
    assert_eq!(config.base_url, "https://custom.api.com");
}

#[tokio::test]
async fn call_missing_query() {
    let config = TavilyConfig::new("key");
    let tool = TavilySearchTool::new(config);
    let result = tool.call(json!({})).await;
    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("query"), "error should mention 'query': {err}");
}

#[tokio::test]
async fn call_non_string_query() {
    let config = TavilyConfig::new("key");
    let tool = TavilySearchTool::new(config);
    let result = tool.call(json!({"query": 42})).await;
    assert!(result.is_err());
}

#[tokio::test]
#[ignore = "requires TAVILY_API_KEY"]
async fn integration_search() {
    let api_key = std::env::var("TAVILY_API_KEY").unwrap();
    let config = TavilyConfig::new(api_key);
    let tool = TavilySearchTool::new(config);
    let result = tool
        .call(json!({"query": "what is Rust programming language"}))
        .await;
    assert!(result.is_ok());
    let output = result.unwrap();
    assert!(output.as_str().unwrap().len() > 10);
}
