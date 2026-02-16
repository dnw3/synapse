use async_trait::async_trait;
use synapse_core::{RunnableConfig, SynapseError};

use crate::runnable::{BoxRunnable, Runnable};

/// Tries the primary runnable first. If it fails, tries each fallback in order.
/// Input must be `Clone` so it can be retried on fallbacks.
pub struct RunnableWithFallbacks<I: Send + Clone + 'static, O: Send + 'static> {
    primary: BoxRunnable<I, O>,
    fallbacks: Vec<BoxRunnable<I, O>>,
}

impl<I: Send + Clone + 'static, O: Send + 'static> RunnableWithFallbacks<I, O> {
    pub fn new(primary: BoxRunnable<I, O>, fallbacks: Vec<BoxRunnable<I, O>>) -> Self {
        Self { primary, fallbacks }
    }
}

#[async_trait]
impl<I: Send + Clone + 'static, O: Send + 'static> Runnable<I, O> for RunnableWithFallbacks<I, O> {
    async fn invoke(&self, input: I, config: &RunnableConfig) -> Result<O, SynapseError> {
        let mut last_error = match self.primary.invoke(input.clone(), config).await {
            Ok(output) => return Ok(output),
            Err(e) => e,
        };
        for fallback in &self.fallbacks {
            match fallback.invoke(input.clone(), config).await {
                Ok(output) => return Ok(output),
                Err(e) => last_error = e,
            }
        }
        Err(last_error)
    }
}
