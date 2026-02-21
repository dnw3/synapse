//! Tavily search tool integration for the Synaptic framework.
//!
//! This crate provides [`TavilySearchTool`], a web search tool that implements
//! the [`Tool`](synaptic_core::Tool) trait using the
//! [Tavily API](https://tavily.com/).
//!
//! # Quick start
//!
//! ```rust,no_run
//! use synaptic_tavily::{TavilySearchTool, TavilyConfig};
//! use synaptic_core::Tool;
//! use serde_json::json;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = TavilyConfig::new("your-api-key")
//!     .with_max_results(3)
//!     .with_search_depth("advanced");
//! let tool = TavilySearchTool::new(config);
//!
//! let result = tool.call(json!({"query": "Rust programming language"})).await?;
//! println!("{}", result);
//! # Ok(())
//! # }
//! ```

mod search;

pub use search::{TavilyConfig, TavilySearchTool};

// Re-export core trait for convenience.
pub use synaptic_core::Tool;
