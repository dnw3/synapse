//! Jina Reader tool — converts any URL to clean Markdown for LLM consumption.

use async_trait::async_trait;
use serde_json::{json, Value};
use synaptic_core::{SynapticError, Tool};

/// Jina Reader tool — converts any URL to clean Markdown for LLM consumption.
///
/// Uses the free Jina Reader API (no API key required). Removes ads, navigation,
/// and boilerplate from web pages, returning clean Markdown text.
///
/// # Example
///
/// ```rust,ignore
/// use synaptic_tools::JinaReaderTool;
/// use synaptic_core::Tool;
///
/// let tool = JinaReaderTool::new();
/// let result = tool.call(serde_json::json!({"url": "https://example.com"})).await?;
/// println!("{}", result["content"].as_str().unwrap());
/// ```
pub struct JinaReaderTool {
    client: reqwest::Client,
}

impl JinaReaderTool {
    /// Create a new `JinaReaderTool`. No API key required.
    pub fn new() -> Self {
        Self {
            client: reqwest::Client::new(),
        }
    }
}

impl Default for JinaReaderTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for JinaReaderTool {
    fn name(&self) -> &'static str {
        "jina_reader"
    }

    fn description(&self) -> &'static str {
        "Convert any web page URL to clean Markdown for LLM consumption. Removes ads, navigation, and boilerplate. Free to use, no API key required."
    }

    fn parameters(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "The URL to fetch and convert to Markdown"
                }
            },
            "required": ["url"]
        }))
    }

    async fn call(&self, args: Value) -> Result<Value, SynapticError> {
        let url = args["url"]
            .as_str()
            .ok_or_else(|| SynapticError::Tool("missing 'url' parameter".to_string()))?;

        let reader_url = format!("https://r.jina.ai/{}", url);
        let resp = self
            .client
            .get(&reader_url)
            .header("Accept", "text/markdown")
            .header("X-Return-Format", "markdown")
            .send()
            .await
            .map_err(|e| SynapticError::Tool(format!("Jina Reader request: {e}")))?;

        let status = resp.status().as_u16();
        let content = resp
            .text()
            .await
            .map_err(|e| SynapticError::Tool(format!("Jina Reader parse: {e}")))?;

        if status != 200 {
            return Err(SynapticError::Tool(format!(
                "Jina Reader error ({})",
                status
            )));
        }

        Ok(json!({
            "url": url,
            "content": content,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_metadata() {
        let tool = JinaReaderTool::new();
        assert_eq!(tool.name(), "jina_reader");
        assert!(!tool.description().is_empty());
    }

    #[test]
    fn tool_schema() {
        let tool = JinaReaderTool::new();
        let schema = tool.parameters().unwrap();
        assert_eq!(schema["type"], "object");
        assert!(schema["properties"]["url"].is_object());
    }

    #[test]
    fn default_impl() {
        let _tool = JinaReaderTool::default();
    }

    #[tokio::test]
    async fn missing_url_returns_error() {
        let tool = JinaReaderTool::new();
        let result = tool.call(json!({})).await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("url"));
    }
}
