use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use synaptic_core::{SynapseError, Tool};

/// A tool wrapper that catches errors and returns them as string values
/// instead of propagating them.
pub struct HandleErrorTool {
    inner: Arc<dyn Tool>,
    handler: Option<Box<dyn Fn(SynapseError) -> String + Send + Sync>>,
}

impl HandleErrorTool {
    /// Wrap a tool with the default error handler (returns `error.to_string()`).
    pub fn new(inner: Arc<dyn Tool>) -> Self {
        Self {
            inner,
            handler: None,
        }
    }

    /// Wrap a tool with a custom error handler function.
    pub fn with_handler(
        inner: Arc<dyn Tool>,
        handler: impl Fn(SynapseError) -> String + Send + Sync + 'static,
    ) -> Self {
        Self {
            inner,
            handler: Some(Box::new(handler)),
        }
    }
}

#[async_trait]
impl Tool for HandleErrorTool {
    fn name(&self) -> &'static str {
        self.inner.name()
    }

    fn description(&self) -> &'static str {
        self.inner.description()
    }

    async fn call(&self, args: Value) -> Result<Value, SynapseError> {
        match self.inner.call(args).await {
            Ok(value) => Ok(value),
            Err(err) => {
                let error_string = match &self.handler {
                    Some(handler) => handler(err),
                    None => err.to_string(),
                };
                Ok(json!(error_string))
            }
        }
    }
}
