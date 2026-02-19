use std::time::Duration;

use async_trait::async_trait;
use serde_json::Value;
use synaptic_core::SynapticError;

use crate::{AgentMiddleware, ToolCallRequest, ToolCaller};

/// Retries failed tool calls with configurable attempts and backoff.
pub struct ToolRetryMiddleware {
    max_retries: usize,
    base_delay: Duration,
}

impl ToolRetryMiddleware {
    pub fn new(max_retries: usize) -> Self {
        Self {
            max_retries,
            base_delay: Duration::from_millis(100),
        }
    }

    pub fn with_base_delay(mut self, delay: Duration) -> Self {
        self.base_delay = delay;
        self
    }
}

#[async_trait]
impl AgentMiddleware for ToolRetryMiddleware {
    async fn wrap_tool_call(
        &self,
        request: ToolCallRequest,
        next: &dyn ToolCaller,
    ) -> Result<Value, SynapticError> {
        let mut last_err = None;
        for attempt in 0..=self.max_retries {
            match next.call(request.clone()).await {
                Ok(val) => return Ok(val),
                Err(e) => {
                    last_err = Some(e);
                    if attempt < self.max_retries {
                        let delay = self.base_delay * 2u32.saturating_pow(attempt as u32);
                        tokio::time::sleep(delay).await;
                    }
                }
            }
        }
        Err(last_err.unwrap())
    }
}
