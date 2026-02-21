# Tavily Search Tool

This guide shows how to use the [Tavily](https://tavily.com/) web search API as a tool in Synaptic. Tavily is a search engine optimized for LLM agents, returning concise and relevant results.

## Setup

Add the `tavily` feature to your `Cargo.toml`:

```toml
[dependencies]
synaptic = { version = "0.2", features = ["openai", "tavily"] }
```

Set your Tavily API key:

```bash
export TAVILY_API_KEY="tvly-..."
```

## Configuration

Create a `TavilyConfig` and build the tool:

```rust,ignore
use synaptic::tavily::{TavilyConfig, TavilySearchTool};

let config = TavilyConfig::new("your-tavily-api-key");
let tool = TavilySearchTool::new(config);
```

### Max results

Control how many search results are returned (default is 5):

```rust,ignore
let config = TavilyConfig::new("your-tavily-api-key")
    .with_max_results(10);
```

### Search depth

Choose between `"basic"` (default) and `"advanced"` search depth. Advanced search performs deeper crawling for more comprehensive results:

```rust,ignore
let config = TavilyConfig::new("your-tavily-api-key")
    .with_search_depth("advanced");
```

## Usage

### As a standalone tool

`TavilySearchTool` implements the `Tool` trait with the name `"tavily_search"`. It accepts a JSON input with a `"query"` field:

```rust,ignore
use synaptic::core::Tool;

let result = tool.call(serde_json::json!({
    "query": "latest Rust programming news"
})).await?;

println!("{}", result);
```

The result is a JSON string containing search results with titles, URLs, and content snippets.

### With an agent

Register the tool with an agent so the LLM can invoke web searches:

```rust,ignore
use std::sync::Arc;
use synaptic::tavily::{TavilyConfig, TavilySearchTool};
use synaptic::tools::ToolRegistry;
use synaptic::graph::create_react_agent;
use synaptic::openai::OpenAiChatModel;

let search = TavilySearchTool::new(TavilyConfig::new("your-tavily-api-key"));

let mut registry = ToolRegistry::new();
registry.register(Arc::new(search));

let model = OpenAiChatModel::new("gpt-4o");
let agent = create_react_agent(Arc::new(model), registry)?;
```

The agent can now call `tavily_search` when it needs to look up current information.

### Tool definition

The tool advertises the following schema to the LLM:

```json
{
  "name": "tavily_search",
  "description": "Search the web for current information on a topic.",
  "parameters": {
    "type": "object",
    "properties": {
      "query": {
        "type": "string",
        "description": "The search query"
      }
    },
    "required": ["query"]
  }
}
```

## Configuration reference

| Field | Type | Default | Description |
|-------|------|---------|-------------|
| `api_key` | `String` | required | Tavily API key |
| `max_results` | `usize` | `5` | Maximum number of search results to return |
| `search_depth` | `String` | `"basic"` | Search depth: `"basic"` or `"advanced"` |
