use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;
use synapse_core::{RunnableConfig, SynapseError};

/// A stream of results from a runnable.
pub type RunnableOutputStream<'a, O> =
    Pin<Box<dyn Stream<Item = Result<O, SynapseError>> + Send + 'a>>;

/// The core composition trait. All LCEL components implement this.
///
/// Implementors only need to provide `invoke`. Default implementations
/// are provided for `batch` (sequential) and `boxed` (type-erased wrapper).
#[async_trait]
pub trait Runnable<I, O>: Send + Sync
where
    I: Send + 'static,
    O: Send + 'static,
{
    /// Execute this runnable on a single input.
    async fn invoke(&self, input: I, config: &RunnableConfig) -> Result<O, SynapseError>;

    /// Execute this runnable on multiple inputs sequentially.
    async fn batch(&self, inputs: Vec<I>, config: &RunnableConfig) -> Vec<Result<O, SynapseError>> {
        let mut results = Vec::with_capacity(inputs.len());
        for input in inputs {
            results.push(self.invoke(input, config).await);
        }
        results
    }

    /// Wrap this runnable into a type-erased `BoxRunnable` for composition via `|`.
    fn boxed(self) -> BoxRunnable<I, O>
    where
        Self: Sized + 'static,
    {
        BoxRunnable {
            inner: Box::new(self),
        }
    }
}

/// A type-erased runnable that supports the `|` pipe operator for composition.
///
/// ```ignore
/// let chain = step1.boxed() | step2.boxed() | step3.boxed();
/// let result = chain.invoke(input, &config).await?;
/// ```
pub struct BoxRunnable<I: Send + 'static, O: Send + 'static> {
    inner: Box<dyn Runnable<I, O>>,
}

impl<I: Send + 'static, O: Send + 'static> BoxRunnable<I, O> {
    pub fn new<R: Runnable<I, O> + 'static>(runnable: R) -> Self {
        Self {
            inner: Box::new(runnable),
        }
    }

    /// Stream the output as a single-item stream wrapping `invoke`.
    pub fn stream<'a>(
        &'a self,
        input: I,
        config: &'a RunnableConfig,
    ) -> RunnableOutputStream<'a, O> {
        Box::pin(async_stream::stream! {
            match self.invoke(input, config).await {
                Ok(output) => yield Ok(output),
                Err(e) => yield Err(e),
            }
        })
    }
}

#[async_trait]
impl<I: Send + 'static, O: Send + 'static> Runnable<I, O> for BoxRunnable<I, O> {
    async fn invoke(&self, input: I, config: &RunnableConfig) -> Result<O, SynapseError> {
        self.inner.invoke(input, config).await
    }

    async fn batch(&self, inputs: Vec<I>, config: &RunnableConfig) -> Vec<Result<O, SynapseError>> {
        self.inner.batch(inputs, config).await
    }
}
