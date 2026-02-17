use std::future::Future;
use std::marker::PhantomData;

use async_trait::async_trait;
use synaptic_core::SynapseError;

use crate::State;

/// A node in the graph that processes state.
#[async_trait]
pub trait Node<S: State>: Send + Sync {
    async fn process(&self, state: S) -> Result<S, SynapseError>;
}

/// Wraps an async function as a Node.
pub struct FnNode<S, F, Fut>
where
    S: State,
    F: Fn(S) -> Fut + Send + Sync,
    Fut: Future<Output = Result<S, SynapseError>> + Send,
{
    func: F,
    _marker: PhantomData<S>,
}

impl<S, F, Fut> FnNode<S, F, Fut>
where
    S: State,
    F: Fn(S) -> Fut + Send + Sync,
    Fut: Future<Output = Result<S, SynapseError>> + Send,
{
    pub fn new(func: F) -> Self {
        Self {
            func,
            _marker: PhantomData,
        }
    }
}

#[async_trait]
impl<S, F, Fut> Node<S> for FnNode<S, F, Fut>
where
    S: State,
    F: Fn(S) -> Fut + Send + Sync,
    Fut: Future<Output = Result<S, SynapseError>> + Send,
{
    async fn process(&self, state: S) -> Result<S, SynapseError> {
        (self.func)(state).await
    }
}
