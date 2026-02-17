use std::pin::Pin;

use async_trait::async_trait;
use futures::{Stream, StreamExt};
use synapse_core::{RunnableConfig, SynapseError};

use crate::runnable::{Runnable, RunnableOutputStream};

type GeneratorFn<I, O> =
    dyn Fn(I) -> Pin<Box<dyn Stream<Item = Result<O, SynapseError>> + Send>> + Send + Sync;

/// A runnable built from a generator function that yields streaming output.
///
/// The generator function receives an input and returns a `Stream` of results.
/// `invoke()` collects the entire stream into the final item, while `stream()`
/// returns the generator's output directly for true streaming.
///
/// ```ignore
/// let gen = RunnableGenerator::new(|input: String| {
///     async_stream::stream! {
///         for ch in input.chars() {
///             yield Ok(ch.to_string());
///         }
///     }
/// });
/// // stream() yields individual characters; invoke() collects them
/// ```
pub struct RunnableGenerator<I: Send + 'static, O: Send + 'static> {
    func: Box<GeneratorFn<I, O>>,
}

impl<I: Send + 'static, O: Send + 'static> RunnableGenerator<I, O> {
    pub fn new<F, S>(func: F) -> Self
    where
        F: Fn(I) -> S + Send + Sync + 'static,
        S: Stream<Item = Result<O, SynapseError>> + Send + 'static,
    {
        Self {
            func: Box::new(move |input| Box::pin(func(input))),
        }
    }
}

#[async_trait]
impl<I: Send + 'static, O: Send + 'static> Runnable<I, Vec<O>> for RunnableGenerator<I, O> {
    async fn invoke(&self, input: I, _config: &RunnableConfig) -> Result<Vec<O>, SynapseError> {
        let stream = (self.func)(input);
        futures::pin_mut!(stream);
        let mut results = Vec::new();
        while let Some(item) = stream.next().await {
            results.push(item?);
        }
        Ok(results)
    }

    fn stream<'a>(
        &'a self,
        input: I,
        _config: &'a RunnableConfig,
    ) -> RunnableOutputStream<'a, Vec<O>>
    where
        I: 'a,
    {
        Box::pin(async_stream::stream! {
            let stream = (self.func)(input);
            futures::pin_mut!(stream);
            while let Some(item) = stream.next().await {
                match item {
                    Ok(val) => yield Ok(vec![val]),
                    Err(e) => yield Err(e),
                }
            }
        })
    }
}
