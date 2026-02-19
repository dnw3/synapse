use std::sync::Arc;

use async_trait::async_trait;
use synaptic_core::{ChatModel, ChatRequest, Message, RunnableConfig, SynapticError};
use synaptic_runnables::Runnable;

/// A parser that uses an LLM to fix outputs that fail to parse,
/// including the original prompt context for better correction.
///
/// Wraps an inner `Runnable<String, O>`. If the inner parser fails,
/// sends the original prompt, the completion, and the error to the LLM
/// and retries parsing.
pub struct RetryOutputParser<O: Send + Sync + 'static> {
    inner: Box<dyn Runnable<String, O>>,
    llm: Arc<dyn ChatModel>,
    prompt: String,
    max_retries: usize,
}

impl<O: Send + Sync + 'static> RetryOutputParser<O> {
    /// Create a new `RetryOutputParser` wrapping the given inner parser, LLM,
    /// and original prompt that generated the output.
    /// Defaults to 1 retry attempt.
    pub fn new(
        inner: Box<dyn Runnable<String, O>>,
        llm: Arc<dyn ChatModel>,
        prompt: impl Into<String>,
    ) -> Self {
        Self {
            inner,
            llm,
            prompt: prompt.into(),
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
impl<O: Send + Sync + 'static> Runnable<String, O> for RetryOutputParser<O> {
    async fn invoke(&self, input: String, config: &RunnableConfig) -> Result<O, SynapticError> {
        // First attempt with the original input.
        match self.inner.invoke(input.clone(), config).await {
            Ok(value) => return Ok(value),
            Err(first_err) => {
                let mut last_err = first_err;
                let mut current_input = input;

                for _ in 0..self.max_retries {
                    let retry_prompt = format!(
                        "Prompt:\n{}\n\nCompletion:\n{}\n\nError:\n{}\n\nPlease provide a corrected completion that will parse successfully.",
                        self.prompt, current_input, last_err
                    );

                    let request = ChatRequest::new(vec![
                        Message::system("You are a helpful assistant that fixes parsing errors."),
                        Message::human(retry_prompt),
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
