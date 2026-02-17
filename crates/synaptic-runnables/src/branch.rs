use async_trait::async_trait;
use synaptic_core::{RunnableConfig, SynapseError};

use crate::runnable::{BoxRunnable, Runnable};

type BranchCondition<I> = Box<dyn Fn(&I) -> bool + Send + Sync>;

/// Routes input to different runnables based on condition functions.
/// The first matching condition's runnable is invoked. If none match,
/// the default runnable is used.
pub struct RunnableBranch<I: Send + 'static, O: Send + 'static> {
    branches: Vec<(BranchCondition<I>, BoxRunnable<I, O>)>,
    default: BoxRunnable<I, O>,
}

impl<I: Send + 'static, O: Send + 'static> RunnableBranch<I, O> {
    pub fn new(
        branches: Vec<(BranchCondition<I>, BoxRunnable<I, O>)>,
        default: BoxRunnable<I, O>,
    ) -> Self {
        Self { branches, default }
    }
}

#[async_trait]
impl<I: Send + 'static, O: Send + 'static> Runnable<I, O> for RunnableBranch<I, O> {
    async fn invoke(&self, input: I, config: &RunnableConfig) -> Result<O, SynapseError> {
        for (condition, runnable) in &self.branches {
            if condition(&input) {
                return runnable.invoke(input, config).await;
            }
        }
        self.default.invoke(input, config).await
    }
}
