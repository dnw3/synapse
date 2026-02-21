//! AWS Bedrock integration for Synaptic.
//!
//! This crate provides [`BedrockChatModel`], an implementation of the
//! [`ChatModel`](synaptic_core::ChatModel) trait backed by
//! [AWS Bedrock](https://aws.amazon.com/bedrock/) via the Converse API.
//!
//! # Example
//!
//! ```rust,no_run
//! use synaptic_bedrock::{BedrockChatModel, BedrockConfig};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = BedrockConfig::new("anthropic.claude-3-5-sonnet-20241022-v2:0")
//!     .with_max_tokens(1024)
//!     .with_temperature(0.7);
//! let model = BedrockChatModel::new(config).await;
//! # Ok(())
//! # }
//! ```

mod chat_model;

pub use chat_model::{BedrockChatModel, BedrockConfig};

// Re-export core traits for convenience.
pub use synaptic_core::{ChatModel, ChatRequest, ChatResponse, Message};
