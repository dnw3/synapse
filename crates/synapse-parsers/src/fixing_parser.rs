use std::sync::Arc;

use async_trait::async_trait;
use synapse_core::{ChatModel, ChatRequest, Message, RunnableConfig, SynapseError};
use synapse_runnables::Runnable;

/// A parser that uses an LLM to fix outputs that fail to parse.
///
/// Wraps an inner `Runnable<String, O>`. If the inner parser fails,
/// sends the output + error to the LLM and retries parsing.
pub struct OutputFixingParser<O: Send + Sync + 'static> {
    inner: Box<dyn Runnable<String, O>>,
    llm: Arc<dyn ChatModel>,
    max_retries: usize,
}

impl<O: Send + Sync + 'static> OutputFixingParser<O> {
    /// Create a new `OutputFixingParser` wrapping the given inner parser and LLM.
    /// Defaults to 1 retry attempt.
    pub fn new(inner: Box<dyn Runnable<String, O>>, llm: Arc<dyn ChatModel>) -> Self {
        Self {
            inner,
            llm,
            max_retries: 1,
        }
    }

    /// Set the maximum number of retry attempts.
    pub fn with_max_retries(mut self, n: usize) -> Self {
        self.max_retries = n;
        self
    }
}

#[async_trait]
impl<O: Send + Sync + 'static> Runnable<String, O> for OutputFixingParser<O> {
    async fn invoke(&self, input: String, config: &RunnableConfig) -> Result<O, SynapseError> {
        // First attempt with the original input.
        match self.inner.invoke(input.clone(), config).await {
            Ok(value) => return Ok(value),
            Err(first_err) => {
                let mut last_err = first_err;
                let mut current_input = input;

                for _ in 0..self.max_retries {
                    let prompt = format!(
                        "The following output failed to parse:\n\n{}\n\nError: {}\n\nPlease provide a corrected version of the output that will parse successfully.",
                        current_input, last_err
                    );

                    let request = ChatRequest::new(vec![
                        Message::system("You are a helpful assistant that fixes parsing errors."),
                        Message::human(prompt),
                    ]);

                    let response = self.llm.chat(request).await?;
                    let fixed = response.message.content().to_string();

                    match self.inner.invoke(fixed.clone(), config).await {
                        Ok(value) => return Ok(value),
                        Err(e) => {
                            last_err = e;
                            current_input = fixed;
                        }
                    }
                }

                Err(last_err)
            }
        }
    }
}
