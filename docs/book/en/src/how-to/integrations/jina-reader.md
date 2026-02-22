# Jina Reader

[Jina Reader](https://jina.ai/reader/) converts any web page URL to clean Markdown for LLM consumption. It strips ads, navigation menus, and boilerplate, returning the main content. No API key is required.

## Setup

```toml
[dependencies]
synaptic = { version = "0.2", features = ["tools"] }
```

No API key required.

## Usage

```rust,ignore
use synaptic::tools::JinaReaderTool;
use synaptic::core::Tool;
use serde_json::json;

let tool = JinaReaderTool::new();

let result = tool.call(json!({
    "url": "https://blog.rust-lang.org/2025/01/01/Rust-1.84.0.html"
})).await?;

println!("{}", result["content"].as_str().unwrap());
```

## With Agent

```rust,ignore
use synaptic::tools::{JinaReaderTool, ToolRegistry};
use std::sync::Arc;

let registry = ToolRegistry::new();
registry.register(Arc::new(JinaReaderTool::new()))?;
```

## Notes

- Jina Reader is free to use without authentication for light usage.
- The returned content is in Markdown format, making it easy to include in LLM prompts.
- For high-volume usage, consider the Jina AI API with a key for higher rate limits.
- The tool adds the `X-Return-Format: markdown` header to request clean Markdown output.
