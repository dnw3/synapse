use std::pin::Pin;

use async_trait::async_trait;
use futures::Stream;
use synaptic_core::{RunnableConfig, SynapseError};

/// A stream of results from a runnable.
pub type RunnableOutputStream<'a, O> =
    Pin<Box<dyn Stream<Item = Result<O, SynapseError>> + Send + 'a>>;

/// The core composition trait. All LCEL components implement this.
///
/// Implementors only need to provide `invoke`. Default implementations
/// are provided for `batch` (sequential), `stream` (wraps invoke), and
/// `boxed` (type-erased wrapper).
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

    /// Stream the output. Default wraps `invoke` as a single-item stream.
    /// Override for true streaming (e.g., token-by-token from an LLM).
    fn stream<'a>(&'a self, input: I, config: &'a RunnableConfig) -> RunnableOutputStream<'a, O>
    where
        I: 'a,
    {
        Box::pin(async_stream::stream! {
            match self.invoke(input, config).await {
                Ok(output) => yield Ok(output),
                Err(e) => yield Err(e),
            }
        })
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

/// Trait object for streaming — used internally to delegate `stream()` through `BoxRunnable`.
trait RunnableStream<I: Send + 'static, O: Send + 'static>: Runnable<I, O> {
    fn stream_boxed<'a>(
        &'a self,
        input: I,
        config: &'a RunnableConfig,
    ) -> RunnableOutputStream<'a, O>
    where
        I: 'a;
}

impl<I: Send + 'static, O: Send + 'static, T: Runnable<I, O>> RunnableStream<I, O> for T {
    fn stream_boxed<'a>(
        &'a self,
        input: I,
        config: &'a RunnableConfig,
    ) -> RunnableOutputStream<'a, O>
    where
        I: 'a,
    {
        self.stream(input, config)
    }
}

/// A type-erased runnable that supports the `|` pipe operator for composition.
///
/// ```ignore
/// let chain = step1.boxed() | step2.boxed() | step3.boxed();
/// let result = chain.invoke(input, &config).await?;
/// ```
pub struct BoxRunnable<I: Send + 'static, O: Send + 'static> {
    inner: Box<dyn RunnableStream<I, O>>,
}

impl<I: Send + 'static, O: Send + 'static> BoxRunnable<I, O> {
    pub fn new<R: Runnable<I, O> + 'static>(runnable: R) -> Self {
        Self {
            inner: Box::new(runnable),
        }
    }

    /// Stream the output, delegating to the inner runnable's `stream()`.
    pub fn stream<'a>(
        &'a self,
        input: I,
        config: &'a RunnableConfig,
    ) -> RunnableOutputStream<'a, O> {
        self.inner.stream_boxed(input, config)
    }

    /// Bind a config transform to this runnable, producing a new `BoxRunnable`
    /// that applies the transform before delegating.
    pub fn bind(
        self,
        transform: impl Fn(RunnableConfig) -> RunnableConfig + Send + Sync + 'static,
    ) -> BoxRunnable<I, O> {
        BoxRunnable::new(RunnableBind {
            inner: self,
            config_transform: Box::new(transform),
        })
    }

    /// Return a new runnable that always uses the given config, ignoring the
    /// config passed at invocation time.
    pub fn with_config(self, config: RunnableConfig) -> BoxRunnable<I, O> {
        self.bind(move |_| config.clone())
    }

    /// Wrap this runnable with before/after listener callbacks.
    pub fn with_listeners(
        self,
        on_start: impl Fn(&RunnableConfig) + Send + Sync + 'static,
        on_end: impl Fn(&RunnableConfig) + Send + Sync + 'static,
    ) -> BoxRunnable<I, O> {
        BoxRunnable::new(RunnableWithListeners {
            inner: self,
            on_start: Box::new(on_start),
            on_end: Box::new(on_end),
        })
    }
}

impl<I: Send + 'static, O: Send + 'static> BoxRunnable<Vec<I>, Vec<O>> {
    /// Shorthand for wrapping this runnable with `RunnableEach`.
    /// Requires `I: Send + 'static, O: Send + 'static` and that the runnable
    /// operates on `Vec<I> -> Vec<O>`.
    ///
    /// See also `RunnableEach::new()` for wrapping a `BoxRunnable<I, O>`.
    pub fn map_each(inner: BoxRunnable<I, O>) -> BoxRunnable<Vec<I>, Vec<O>> {
        BoxRunnable::new(crate::each::RunnableEach::new(inner))
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

    fn stream<'a>(&'a self, input: I, config: &'a RunnableConfig) -> RunnableOutputStream<'a, O>
    where
        I: 'a,
    {
        self.inner.stream_boxed(input, config)
    }
}

/// A runnable that applies a config transform before delegating to the inner runnable.
struct RunnableBind<I: Send + 'static, O: Send + 'static> {
    inner: BoxRunnable<I, O>,
    config_transform: Box<dyn Fn(RunnableConfig) -> RunnableConfig + Send + Sync>,
}

#[async_trait]
impl<I: Send + 'static, O: Send + 'static> Runnable<I, O> for RunnableBind<I, O> {
    async fn invoke(&self, input: I, config: &RunnableConfig) -> Result<O, SynapseError> {
        let transformed = (self.config_transform)(config.clone());
        self.inner.invoke(input, &transformed).await
    }

    fn stream<'a>(&'a self, input: I, config: &'a RunnableConfig) -> RunnableOutputStream<'a, O>
    where
        I: 'a,
    {
        Box::pin(async_stream::stream! {
            let transformed = (self.config_transform)(config.clone());
            let mut inner_stream = std::pin::pin!(self.inner.stream(input, &transformed));
            use futures::StreamExt;
            while let Some(item) = inner_stream.next().await {
                yield item;
            }
        })
    }
}

/// A runnable that fires listener callbacks before and after invocation.
struct RunnableWithListeners<I: Send + 'static, O: Send + 'static> {
    inner: BoxRunnable<I, O>,
    on_start: Box<dyn Fn(&RunnableConfig) + Send + Sync>,
    on_end: Box<dyn Fn(&RunnableConfig) + Send + Sync>,
}

#[async_trait]
impl<I: Send + 'static, O: Send + 'static> Runnable<I, O> for RunnableWithListeners<I, O> {
    async fn invoke(&self, input: I, config: &RunnableConfig) -> Result<O, SynapseError> {
        (self.on_start)(config);
        let result = self.inner.invoke(input, config).await;
        (self.on_end)(config);
        result
    }

    fn stream<'a>(&'a self, input: I, config: &'a RunnableConfig) -> RunnableOutputStream<'a, O>
    where
        I: 'a,
    {
        Box::pin(async_stream::stream! {
            (self.on_start)(config);
            use futures::StreamExt;
            let mut s = std::pin::pin!(self.inner.stream(input, config));
            while let Some(item) = s.next().await {
                yield item;
            }
            (self.on_end)(config);
        })
    }
}
