use async_trait::async_trait;
use serde_json::Value;
use synaptic_core::{RunnableConfig, SynapseError};

use crate::assign::RunnableAssign;
use crate::runnable::BoxRunnable;
use crate::Runnable;

/// Passes the input through unchanged. Useful in parallel compositions
/// where one branch should preserve the original input.
#[derive(Debug, Clone, Copy, Default)]
pub struct RunnablePassthrough;

impl RunnablePassthrough {
    /// Create a `RunnableAssign` that passes input through and merges additional computed keys.
    pub fn assign(branches: Vec<(String, BoxRunnable<Value, Value>)>) -> RunnableAssign {
        RunnableAssign::new(branches)
    }
}

#[async_trait]
impl<T> Runnable<T, T> for RunnablePassthrough
where
    T: Send + Sync + 'static,
{
    async fn invoke(&self, input: T, _config: &RunnableConfig) -> Result<T, SynapseError> {
        Ok(input)
    }
}
