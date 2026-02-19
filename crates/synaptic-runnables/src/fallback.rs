use async_trait::async_trait;
use synaptic_core::{RunnableConfig, SynapticError};

use crate::runnable::{BoxRunnable, Runnable, RunnableOutputStream};

/// Tries the primary runnable first. If it fails, tries each fallback in order.
/// Input must be `Clone` so it can be retried on fallbacks.
pub struct RunnableWithFallbacks<I: Send + Clone + 'static, O: Send + 'static> {
    primary: BoxRunnable<I, O>,
    fallbacks: Vec<BoxRunnable<I, O>>,
}

impl<I: Send + Clone + 'static, O: Send + 'static> RunnableWithFallbacks<I, O> {
    pub fn new(primary: BoxRunnable<I, O>, fallbacks: Vec<BoxRunnable<I, O>>) -> Self {
        Self { primary, fallbacks }
    }
}

#[async_trait]
impl<I: Send + Clone + 'static, O: Send + 'static> Runnable<I, O> for RunnableWithFallbacks<I, O> {
    async fn invoke(&self, input: I, config: &RunnableConfig) -> Result<O, SynapticError> {
        let mut last_error = match self.primary.invoke(input.clone(), config).await {
            Ok(output) => return Ok(output),
            Err(e) => e,
        };
        for fallback in &self.fallbacks {
            match fallback.invoke(input.clone(), config).await {
                Ok(output) => return Ok(output),
                Err(e) => last_error = e,
            }
        }
        Err(last_error)
    }

    /// Stream: try primary stream, fall back on error.
    fn stream<'a>(&'a self, input: I, config: &'a RunnableConfig) -> RunnableOutputStream<'a, O>
    where
        I: 'a,
    {
        Box::pin(async_stream::stream! {
            use futures::StreamExt;

            // Try primary
            let mut primary_stream = std::pin::pin!(self.primary.stream(input.clone(), config));
            let mut primary_items = Vec::new();
            let mut primary_failed = false;

            while let Some(item) = primary_stream.next().await {
                match item {
                    Ok(val) => primary_items.push(val),
                    Err(_e) => {
                        primary_failed = true;
                        break;
                    }
                }
            }

            if !primary_failed {
                for item in primary_items {
                    yield Ok(item);
                }
                return;
            }

            // Try fallbacks
            let mut last_error = None;
            for fallback in &self.fallbacks {
                let mut fb_stream = std::pin::pin!(fallback.stream(input.clone(), config));
                let mut fb_items = Vec::new();
                let mut fb_failed = false;

                while let Some(item) = fb_stream.next().await {
                    match item {
                        Ok(val) => fb_items.push(val),
                        Err(e) => {
                            fb_failed = true;
                            last_error = Some(e);
                            break;
                        }
                    }
                }

                if !fb_failed {
                    for item in fb_items {
                        yield Ok(item);
                    }
                    return;
                }
            }

            if let Some(e) = last_error {
                yield Err(e);
            }
        })
    }
}
