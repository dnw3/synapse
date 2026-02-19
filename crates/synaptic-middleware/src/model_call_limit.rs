use std::sync::atomic::{AtomicUsize, Ordering};

use async_trait::async_trait;
use synaptic_core::SynapticError;

use crate::{AgentMiddleware, ModelCaller, ModelRequest, ModelResponse};

/// Limits the number of model invocations during a single agent run.
///
/// When the limit is exceeded, `wrap_model_call` returns a
/// `SynapticError::MaxStepsExceeded` error.
pub struct ModelCallLimitMiddleware {
    max_calls: usize,
    count: AtomicUsize,
}

impl ModelCallLimitMiddleware {
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
impl AgentMiddleware for ModelCallLimitMiddleware {
    async fn wrap_model_call(
        &self,
        request: ModelRequest,
        next: &dyn ModelCaller,
    ) -> Result<ModelResponse, SynapticError> {
        let current = self.count.fetch_add(1, Ordering::SeqCst);
        if current >= self.max_calls {
            return Err(SynapticError::MaxStepsExceeded {
                max_steps: self.max_calls,
            });
        }
        next.call(request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tracks_count() {
        let mw = ModelCallLimitMiddleware::new(5);
        assert_eq!(mw.call_count(), 0);
        mw.count.fetch_add(1, Ordering::SeqCst);
        assert_eq!(mw.call_count(), 1);
        mw.reset();
        assert_eq!(mw.call_count(), 0);
    }
}
