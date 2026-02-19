use std::future::Future;
use std::marker::PhantomData;

use async_trait::async_trait;
use synaptic_core::SynapticError;

use crate::command::NodeOutput;
use crate::State;

/// A node in the graph that processes state.
///
/// Nodes return `NodeOutput<S>` which is either a state update
/// or a `Command` for dynamic control flow.
///
/// For backwards compatibility, returning `Ok(state.into())` works
/// via the `From<S> for NodeOutput<S>` impl.
#[async_trait]
pub trait Node<S: State>: Send + Sync {
    async fn process(&self, state: S) -> Result<NodeOutput<S>, SynapticError>;
}

/// Wraps an async function as a Node.
///
/// The function can return either `NodeOutput<S>` directly or `S`
/// (via `Into<NodeOutput<S>>`).
pub struct FnNode<S, F, Fut>
where
    S: State,
    F: Fn(S) -> Fut + Send + Sync,
    Fut: Future<Output = Result<NodeOutput<S>, SynapticError>> + Send,
{
    func: F,
    _marker: PhantomData<S>,
}

impl<S, F, Fut> FnNode<S, F, Fut>
where
    S: State,
    F: Fn(S) -> Fut + Send + Sync,
    Fut: Future<Output = Result<NodeOutput<S>, SynapticError>> + Send,
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
    Fut: Future<Output = Result<NodeOutput<S>, SynapticError>> + Send,
{
    async fn process(&self, state: S) -> Result<NodeOutput<S>, SynapticError> {
        (self.func)(state).await
    }
}
