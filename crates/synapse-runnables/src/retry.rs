use std::time::Duration;

use async_trait::async_trait;
use synapse_core::{RunnableConfig, SynapseError};

use crate::runnable::{BoxRunnable, Runnable};

/// Retry policy configuration for `RunnableRetry`.
///
/// Controls how many times to retry, the backoff schedule, and which errors
/// are eligible for retrying.
pub struct RetryPolicy {
    /// Maximum number of attempts (including the initial attempt).
    pub max_attempts: usize,
    /// Base delay for exponential backoff. The actual delay for attempt `n` is
    /// `min(base_delay * 2^n, max_delay)`.
    pub base_delay: Duration,
    /// Upper bound on the backoff delay.
    pub max_delay: Duration,
    /// Optional predicate to decide if an error is retryable.
    /// When `None`, all errors are retried.
    retry_on: Option<Box<dyn Fn(&SynapseError) -> bool + Send + Sync>>,
}

impl std::fmt::Debug for RetryPolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RetryPolicy")
            .field("max_attempts", &self.max_attempts)
            .field("base_delay", &self.base_delay)
            .field("max_delay", &self.max_delay)
            .field("retry_on", &self.retry_on.as_ref().map(|_| "..."))
            .finish()
    }
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            base_delay: Duration::from_millis(100),
            max_delay: Duration::from_secs(10),
            retry_on: None,
        }
    }
}

impl RetryPolicy {
    /// Set the maximum number of attempts (including the initial attempt).
    pub fn with_max_attempts(mut self, max_attempts: usize) -> Self {
        self.max_attempts = max_attempts;
        self
    }

    /// Set the base delay for exponential backoff.
    pub fn with_base_delay(mut self, base_delay: Duration) -> Self {
        self.base_delay = base_delay;
        self
    }

    /// Set the upper bound on the backoff delay.
    pub fn with_max_delay(mut self, max_delay: Duration) -> Self {
        self.max_delay = max_delay;
        self
    }

    /// Set a predicate to decide which errors are retryable.
    /// When not set, all errors are retried.
    pub fn with_retry_on(
        mut self,
        predicate: impl Fn(&SynapseError) -> bool + Send + Sync + 'static,
    ) -> Self {
        self.retry_on = Some(Box::new(predicate));
        self
    }

    /// Compute the backoff delay for the given attempt (0-indexed).
    fn delay_for_attempt(&self, attempt: usize) -> Duration {
        let delay = self.base_delay.saturating_mul(1 << attempt);
        std::cmp::min(delay, self.max_delay)
    }

    /// Check whether the given error should be retried.
    fn should_retry(&self, error: &SynapseError) -> bool {
        match &self.retry_on {
            Some(predicate) => predicate(error),
            None => true,
        }
    }
}

/// Wraps a runnable with configurable retry logic and exponential backoff.
///
/// The input type must be `Clone` because the input is re-used for each retry attempt.
///
/// ```ignore
/// let policy = RetryPolicy::default()
///     .with_max_attempts(5)
///     .with_base_delay(Duration::from_millis(200));
/// let retrying = RunnableRetry::new(flaky_step.boxed(), policy);
/// let result = retrying.invoke(input, &config).await?;
/// ```
pub struct RunnableRetry<I: Send + Clone + 'static, O: Send + 'static> {
    inner: BoxRunnable<I, O>,
    policy: RetryPolicy,
}

impl<I: Send + Clone + 'static, O: Send + 'static> RunnableRetry<I, O> {
    pub fn new(inner: BoxRunnable<I, O>, policy: RetryPolicy) -> Self {
        Self { inner, policy }
    }
}

#[async_trait]
impl<I: Send + Clone + 'static, O: Send + 'static> Runnable<I, O> for RunnableRetry<I, O> {
    async fn invoke(&self, input: I, config: &RunnableConfig) -> Result<O, SynapseError> {
        let mut last_error: Option<SynapseError> = None;

        for attempt in 0..self.policy.max_attempts {
            let input_clone = input.clone();
            match self.inner.invoke(input_clone, config).await {
                Ok(output) => return Ok(output),
                Err(e) => {
                    let is_last_attempt = attempt + 1 >= self.policy.max_attempts;
                    if is_last_attempt || !self.policy.should_retry(&e) {
                        return Err(e);
                    }

                    let delay = self.policy.delay_for_attempt(attempt);
                    tokio::time::sleep(delay).await;
                    last_error = Some(e);
                }
            }
        }

        // This is only reached when max_attempts is 0.
        Err(last_error.unwrap_or_else(|| {
            SynapseError::Config("RunnableRetry: max_attempts must be >= 1".into())
        }))
    }
}
