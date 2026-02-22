//! Brave Search API tool for privacy-focused web search.

use async_trait::async_trait;
use serde_json::{json, Value};
use synaptic_core::{SynapticError, Tool};

/// Brave Search API tool for web search with privacy focus.
///
/// Requires a Brave Search API key. Get one from <https://brave.com/search/api/>.
///
/// # Example
///
/// ```rust,ignore
/// use synaptic_tools::BraveSearchTool;
/// use synaptic_core::Tool;
///
/// let tool = BraveSearchTool::new("your-api-key").with_max_results(5);
/// let result = tool.call(serde_json::json!({"query": "Rust async runtime"})).await?;
/// ```
pub struct BraveSearchTool {
    client: reqwest::Client,
    api_key: String,
    max_results: usize,
}

impl BraveSearchTool {
    /// Create a new `BraveSearchTool` with the given API key.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            client: reqwest::Client::new(),
            api_key: api_key.into(),
            max_results: 5,
        }
    }

    /// Set the maximum number of results to return.
    pub fn with_max_results(mut self, n: usize) -> Self {
        self.max_results = n;
        self
    }
}

#[async_trait]
impl Tool for BraveSearchTool {
    fn name(&self) -> &'static str {
        "brave_search"
    }

    fn description(&self) -> &'static str {
        "Search the web using Brave Search API. Returns titles, URLs, and descriptions of relevant results."
    }

    fn parameters(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query"
                }
            },
            "required": ["query"]
        }))
    }

    async fn call(&self, args: Value) -> Result<Value, SynapticError> {
        let query = args["query"]
            .as_str()
            .ok_or_else(|| SynapticError::Tool("missing 'query' parameter".to_string()))?;

        let resp = self
            .client
            .get("https://api.search.brave.com/res/v1/web/search")
            .query(&[("q", query), ("count", &self.max_results.to_string())])
            .header("X-Subscription-Token", &self.api_key)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|e| SynapticError::Tool(format!("Brave Search request: {e}")))?;

        let status = resp.status().as_u16();
        let body: Value = resp
            .json()
            .await
            .map_err(|e| SynapticError::Tool(format!("Brave Search parse: {e}")))?;

        if status != 200 {
            return Err(SynapticError::Tool(format!(
                "Brave Search error ({}): {}",
                status, body
            )));
        }

        let results = body["web"]["results"]
            .as_array()
            .map(|arr| {
                arr.iter()
                    .map(|r| {
                        json!({
                            "title": r["title"],
                            "url": r["url"],
                            "description": r["description"],
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        Ok(json!({ "query": query, "results": results }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_metadata() {
        let tool = BraveSearchTool::new("test-key");
        assert_eq!(tool.name(), "brave_search");
        assert!(!tool.description().is_empty());
        assert_eq!(tool.max_results, 5);
    }

    #[test]
    fn tool_schema() {
        let tool = BraveSearchTool::new("test-key");
        let schema = tool.parameters().unwrap();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["query"].is_object());
    }

    #[test]
    fn builder_max_results() {
        let tool = BraveSearchTool::new("test-key").with_max_results(10);
        assert_eq!(tool.max_results, 10);
    }

    #[tokio::test]
    async fn missing_query_returns_error() {
        let tool = BraveSearchTool::new("test-key");
        let result = tool.call(json!({})).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("query"));
    }
}
