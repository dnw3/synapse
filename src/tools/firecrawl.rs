//! Firecrawl web scraping tool for the Deep Agent.
//!
//! Uses the Firecrawl API to scrape web pages and return their content
//! as clean markdown. Requires the FIRECRAWL_API_KEY environment variable.

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use synaptic::core::{SynapticError, Tool};

/// Tool that scrapes web pages using the Firecrawl API and returns markdown content.
pub struct FirecrawlTool {
    client: reqwest::Client,
}

#[allow(clippy::new_ret_no_self)]
impl FirecrawlTool {
    pub fn new() -> Arc<dyn Tool> {
        Arc::new(Self {
            client: reqwest::Client::new(),
        })
    }
}

#[async_trait]
impl Tool for FirecrawlTool {
    fn name(&self) -> &'static str {
        "firecrawl_scrape"
    }

    fn description(&self) -> &'static str {
        "Scrape a web page and return its content as clean markdown. Useful for reading documentation, articles, and other web content."
    }

    fn parameters(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL of the web page to scrape."
                },
                "formats": {
                    "type": "array",
                    "items": { "type": "string" },
                    "description": "Output formats to request (e.g. [\"markdown\", \"html\"]). Defaults to [\"markdown\"]."
                },
                "only_main_content": {
                    "type": "boolean",
                    "description": "If true, extract only the main content (skip navbars, footers, etc.). Defaults to true."
                }
            },
            "required": ["url"]
        }))
    }

    async fn call(&self, args: Value) -> Result<Value, SynapticError> {
        let url = args
            .get("url")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SynapticError::Tool("missing 'url' argument".into()))?;

        let api_key = std::env::var("FIRECRAWL_API_KEY").map_err(|_| {
            SynapticError::Tool(
                "FIRECRAWL_API_KEY environment variable is not set. \
                 Get your API key at https://firecrawl.dev and set it as FIRECRAWL_API_KEY."
                    .into(),
            )
        })?;

        let formats = args
            .get("formats")
            .cloned()
            .unwrap_or_else(|| json!(["markdown"]));

        let only_main_content = args
            .get("only_main_content")
            .and_then(|v| v.as_bool())
            .unwrap_or(true);

        tracing::info!(url = %url, "web scrape started");

        let body = json!({
            "url": url,
            "formats": formats,
            "onlyMainContent": only_main_content,
        });

        let response = self
            .client
            .post("https://api.firecrawl.dev/v1/scrape")
            .header("Authorization", format!("Bearer {}", api_key))
            .header("Content-Type", "application/json")
            .json(&body)
            .send()
            .await
            .map_err(|e| SynapticError::Tool(format!("Firecrawl request failed: {}", e)))?;

        let status = response.status();
        let response_body: Value = response.json().await.map_err(|e| {
            SynapticError::Tool(format!("Failed to parse Firecrawl response: {}", e))
        })?;

        if !status.is_success() {
            let error_msg = response_body
                .get("error")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown error");
            return Err(SynapticError::Tool(format!(
                "Firecrawl API error ({}): {}",
                status, error_msg
            )));
        }

        let markdown = response_body
            .get("data")
            .and_then(|d| d.get("markdown"))
            .and_then(|m| m.as_str())
            .unwrap_or("");

        Ok(json!({
            "url": url,
            "content": markdown,
        }))
    }
}
