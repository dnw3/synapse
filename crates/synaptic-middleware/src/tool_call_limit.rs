use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use serde_json::Value;
use synaptic_core::SynapticError;

use crate::{AgentMiddleware, ToolCallRequest, ToolCaller};

/// Limits the number of tool invocations during a single agent run.
///
/// When the limit is exceeded, `wrap_tool_call` returns a
/// `SynapticError::MaxStepsExceeded` error.
pub struct ToolCallLimitMiddleware {
    max_calls: usize,
    count: AtomicUsize,
}

impl ToolCallLimitMiddleware {
    pub fn new(max_calls: usize) -> Self {
        Self {
            max_calls,
            count: AtomicUsize::new(0),
        }
    }

    pub fn call_count(&self) -> usize {
        self.count.load(Ordering::SeqCst)
    }

    pub fn reset(&self) {
        self.count.store(0, Ordering::SeqCst);
    }
}

#[async_trait]
impl AgentMiddleware for ToolCallLimitMiddleware {
    async fn wrap_tool_call(
        &self,
        request: ToolCallRequest,
        next: &dyn ToolCaller,
    ) -> Result<Value, SynapticError> {
        let current = self.count.fetch_add(1, Ordering::SeqCst);
        if current >= self.max_calls {
            return Err(SynapticError::MaxStepsExceeded {
                max_steps: self.max_calls,
            });
        }
        next.call(request).await
    }
}
