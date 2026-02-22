//! Feishu/Lark integration for the Synaptic framework.
//!
//! This crate provides first-class integration with the Feishu (Lark) Open Platform:
//!
//! - [`LarkDocLoader`] — load documents and Wiki pages into Synaptic [`Document`]s
//! - [`LarkMessageTool`] — send messages to chats and users as an Agent tool
//! - [`LarkBitableTool`] — search, create, and update rows in Bitable (multi-dim tables)
//!
//! # Quick start
//!
//! ```toml
//! [dependencies]
//! synaptic = { version = "0.2", features = ["lark"] }
//! ```
//!
//! ```rust,no_run
//! use synaptic_lark::{LarkConfig, LarkDocLoader};
//! use synaptic_core::Loader;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let config = LarkConfig::new("cli_xxx", "app_secret_xxx");
//! let loader = LarkDocLoader::new(config)
//!     .with_doc_tokens(vec!["doxcnAbcXxx".to_string()]);
//! let docs = loader.load().await?;
//! # Ok(())
//! # }
//! ```

mod auth;
pub mod loaders;
pub mod tools;

pub use loaders::doc::LarkDocLoader;
pub use tools::bitable::LarkBitableTool;
pub use tools::message::LarkMessageTool;

// Re-export core traits for convenience
pub use synaptic_core::{Loader, Tool};

use std::sync::Arc;

/// Configuration for the Lark Open Platform.
///
/// Obtain `app_id` and `app_secret` from the [Lark Developer Console](https://open.feishu.cn/app).
#[derive(Debug, Clone)]
pub struct LarkConfig {
    /// Application ID (`cli_xxx`).
    pub app_id: String,
    /// Application secret.
    pub app_secret: String,
    /// Base URL — use `"https://open.feishu.cn/open-apis"` for public cloud (default)
    /// or `"https://fsopen.bytedance.net/open-apis"` for ByteDance internal network.
    pub base_url: String,
}

impl LarkConfig {
    /// Create a new config targeting the Feishu public cloud.
    pub fn new(app_id: impl Into<String>, app_secret: impl Into<String>) -> Self {
        Self {
            app_id: app_id.into(),
            app_secret: app_secret.into(),
            base_url: "https://open.feishu.cn/open-apis".to_string(),
        }
    }

    /// Override the base URL (e.g. for ByteDance internal or testing).
    pub fn with_base_url(mut self, url: impl Into<String>) -> Self {
        self.base_url = url.into();
        self
    }

    /// Create a [`TokenCache`] backed by this config.
    pub(crate) fn token_cache(self) -> auth::TokenCache {
        auth::TokenCache::new(Arc::new(self))
    }
}
