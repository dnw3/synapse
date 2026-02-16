use std::ops::BitOr;

use async_trait::async_trait;
use synapse_core::{RunnableConfig, SynapseError};

use crate::runnable::{BoxRunnable, Runnable};

/// Chains two runnables: output of `first` feeds into `second`.
/// Created automatically via the `|` operator on `BoxRunnable`.
pub struct RunnableSequence<I, M, O>
where
    I: Send + 'static,
    M: Send + 'static,
    O: Send + 'static,
{
    pub(crate) first: BoxRunnable<I, M>,
    pub(crate) second: BoxRunnable<M, O>,
}

#[async_trait]
impl<I, M, O> Runnable<I, O> for RunnableSequence<I, M, O>
where
    I: Send + 'static,
    M: Send + 'static,
    O: Send + 'static,
{
    async fn invoke(&self, input: I, config: &RunnableConfig) -> Result<O, SynapseError> {
        let mid = self.first.invoke(input, config).await?;
        self.second.invoke(mid, config).await
    }
}

/// Pipe operator: `a | b` creates a `RunnableSequence` that runs `a` then `b`.
impl<I, M, O> BitOr<BoxRunnable<M, O>> for BoxRunnable<I, M>
where
    I: Send + 'static,
    M: Send + 'static,
    O: Send + 'static,
{
    type Output = BoxRunnable<I, O>;

    fn bitor(self, rhs: BoxRunnable<M, O>) -> BoxRunnable<I, O> {
        BoxRunnable::new(RunnableSequence {
            first: self,
            second: rhs,
        })
    }
}
