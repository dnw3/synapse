use async_trait::async_trait;
use synapse_core::{RunnableConfig, SynapseError};

use crate::runnable::{BoxRunnable, Runnable};

/// Maps a runnable over each element in a list input, producing a list of outputs.
///
/// Similar to Python's `map()`, this applies the inner runnable to every item
/// in the input `Vec`, collecting the results into an output `Vec`.
///
/// ```ignore
/// let upper = RunnableLambda::new(|s: String| async move {
///     Ok(s.to_uppercase())
/// });
/// let each = RunnableEach::new(upper.boxed());
/// let results = each.invoke(vec!["hello".into(), "world".into()], &config).await?;
/// // results == vec!["HELLO", "WORLD"]
/// ```
pub struct RunnableEach<I: Send + 'static, O: Send + 'static> {
    inner: BoxRunnable<I, O>,
}

impl<I: Send + 'static, O: Send + 'static> RunnableEach<I, O> {
    pub fn new(inner: BoxRunnable<I, O>) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl<I: Send + 'static, O: Send + 'static> Runnable<Vec<I>, Vec<O>> for RunnableEach<I, O> {
    async fn invoke(&self, input: Vec<I>, config: &RunnableConfig) -> Result<Vec<O>, SynapseError> {
        let mut results = Vec::with_capacity(input.len());
        for item in input {
            results.push(self.inner.invoke(item, config).await?);
        }
        Ok(results)
    }
}
