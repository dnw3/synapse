use async_trait::async_trait;
use synapse_core::{RunnableConfig, SynapseError};

use crate::Runnable;

/// Passes the input through unchanged. Useful in parallel compositions
/// where one branch should preserve the original input.
#[derive(Debug, Clone, Copy, Default)]
pub struct RunnablePassthrough;

#[async_trait]
impl<T> Runnable<T, T> for RunnablePassthrough
where
    T: Send + Sync + 'static,
{
    async fn invoke(&self, input: T, _config: &RunnableConfig) -> Result<T, SynapseError> {
        Ok(input)
    }
}
