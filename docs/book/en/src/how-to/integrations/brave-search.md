# Brave Search

[Brave Search](https://search.brave.com/) provides privacy-focused web search with an independent index. The `BraveSearchTool` integrates Brave's Web Search API into Synaptic agents.

## Setup

```toml
[dependencies]
synaptic = { version = "0.2", features = ["tools"] }
```

Get an API key from [brave.com/search/api](https://brave.com/search/api/).

## Usage

```rust,ignore
use synaptic::tools::BraveSearchTool;
use synaptic::core::Tool;
use serde_json::json;

let tool = BraveSearchTool::new("your-api-key")
    .with_max_results(5);

let result = tool.call(json!({"query": "Rust async runtime comparison"})).await?;
println!("{}", serde_json::to_string_pretty(&result)?);
```

## With Agent

```rust,ignore
use synaptic::tools::{BraveSearchTool, ToolRegistry};
use std::sync::Arc;

let registry = ToolRegistry::new();
registry.register(Arc::new(BraveSearchTool::new("your-api-key")))?;
```

## Configuration

| Option | Default | Description |
|---|---|---|
| `with_max_results(n)` | `5` | Maximum number of search results to return |

## Notes

- Results include title, URL, and description for each web result.
- The Brave Search API has both free and paid tiers. Check [brave.com/search/api](https://brave.com/search/api/) for rate limits.
- Brave Search maintains an independent index, making it a good alternative to Google for privacy-conscious deployments.
