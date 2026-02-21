use async_trait::async_trait;
use serde_json::{json, Value};
use synaptic_core::{SynapticError, Tool};

/// Configuration for [`TavilySearchTool`].
#[derive(Debug, Clone)]
pub struct TavilyConfig {
    /// Tavily API key.
    pub api_key: String,
    /// Maximum number of search results to return. Defaults to 5.
    pub max_results: usize,
    /// Search depth: `"basic"` or `"advanced"`. Defaults to `"basic"`.
    pub search_depth: String,
    /// Whether to include a direct answer in the response. Defaults to `true`.
    pub include_answer: bool,
    /// Base URL for the Tavily API. Defaults to `"https://api.tavily.com"`.
    pub base_url: String,
}

impl TavilyConfig {
    /// Create a new configuration with the given API key and sensible defaults.
    pub fn new(api_key: impl Into<String>) -> Self {
        Self {
            api_key: api_key.into(),
            max_results: 5,
            search_depth: "basic".to_string(),
            include_answer: true,
            base_url: "https://api.tavily.com".to_string(),
        }
    }

    /// Set the maximum number of results.
    pub fn with_max_results(mut self, n: usize) -> Self {
        self.max_results = n;
        self
    }

    /// Set the search depth (`"basic"` or `"advanced"`).
    pub fn with_search_depth(mut self, depth: impl Into<String>) -> Self {
        self.search_depth = depth.into();
        self
    }

    /// Set whether to include a direct answer.
    pub fn with_include_answer(mut self, include: bool) -> Self {
        self.include_answer = include;
        self
    }

    /// Set a custom base URL (useful for testing with a mock server).
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }
}

/// Tavily web search tool implementing the [`Tool`](synaptic_core::Tool) trait.
///
/// Sends search queries to the Tavily API and returns formatted results
/// suitable for consumption by an LLM.
pub struct TavilySearchTool {
    config: TavilyConfig,
    client: reqwest::Client,
}

impl TavilySearchTool {
    /// Create a new `TavilySearchTool` with the given configuration.
    pub fn new(config: TavilyConfig) -> Self {
        Self {
            config,
            client: reqwest::Client::new(),
        }
    }
}

#[async_trait]
impl Tool for TavilySearchTool {
    fn name(&self) -> &'static str {
        "tavily_search"
    }

    fn description(&self) -> &'static str {
        "Search the web using Tavily API. Input should be a search query string."
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
        let query = args
            .get("query")
            .and_then(|q| q.as_str())
            .ok_or_else(|| SynapticError::Tool("missing 'query' argument".to_string()))?;

        let body = json!({
            "api_key": self.config.api_key,
            "query": query,
            "max_results": self.config.max_results,
            "search_depth": self.config.search_depth,
            "include_answer": self.config.include_answer,
        });

        let response = self
            .client
            .post(format!("{}/search", self.config.base_url))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| SynapticError::Tool(format!("Tavily request failed: {e}")))?;

        if !response.status().is_success() {
            let status = response.status().as_u16();
            let text = response.text().await.unwrap_or_default();
            return Err(SynapticError::Tool(format!(
                "Tavily API error ({status}): {text}"
            )));
        }

        let resp_body: Value = response
            .json()
            .await
            .map_err(|e| SynapticError::Tool(format!("Tavily parse error: {e}")))?;

        // Format results for the LLM
        let mut output = String::new();

        if let Some(answer) = resp_body.get("answer").and_then(|a| a.as_str()) {
            output.push_str(&format!("Answer: {answer}\n\n"));
        }

        if let Some(results) = resp_body.get("results").and_then(|r| r.as_array()) {
            for (i, result) in results.iter().enumerate() {
                let title = result.get("title").and_then(|t| t.as_str()).unwrap_or("");
                let url = result.get("url").and_then(|u| u.as_str()).unwrap_or("");
                let content = result
                    .get("content")
                    .and_then(|c| c.as_str())
                    .unwrap_or("");
                output.push_str(&format!(
                    "{}. {}\n   URL: {}\n   {}\n\n",
                    i + 1,
                    title,
                    url,
                    content
                ));
            }
        }

        Ok(json!(output.trim()))
    }
}
