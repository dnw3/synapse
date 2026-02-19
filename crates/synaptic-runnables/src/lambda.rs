use std::future::Future;
use std::pin::Pin;

use async_trait::async_trait;
use synaptic_core::{RunnableConfig, SynapticError};

use crate::Runnable;

type AsyncFn<I, O> =
    dyn Fn(I) -> Pin<Box<dyn Future<Output = Result<O, SynapticError>> + Send>> + Send + Sync;

/// Wraps an async closure as a `Runnable`.
///
/// ```ignore
/// let upper = RunnableLambda::new(|s: String| async move {
///     Ok(s.to_uppercase())
/// });
/// ```
pub struct RunnableLambda<I: Send + 'static, O: Send + 'static> {
    func: Box<AsyncFn<I, O>>,
}

impl<I: Send + 'static, O: Send + 'static> RunnableLambda<I, O> {
    pub fn new<F, Fut>(func: F) -> Self
    where
        F: Fn(I) -> Fut + Send + Sync + 'static,
        Fut: Future<Output = Result<O, SynapticError>> + Send + 'static,
    {
        Self {
            func: Box::new(move |input| Box::pin(func(input))),
        }
    }
}

#[async_trait]
impl<I: Send + 'static, O: Send + 'static> Runnable<I, O> for RunnableLambda<I, O> {
    async fn invoke(&self, input: I, _config: &RunnableConfig) -> Result<O, SynapticError> {
        (self.func)(input).await
    }
}
