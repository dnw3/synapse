use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use synaptic_core::SynapticError;

/// Result of a single evaluation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalResult {
    /// Score between 0.0 and 1.0.
    pub score: f64,
    /// Whether the evaluation passed.
    pub passed: bool,
    /// Optional reasoning for the result.
    pub reasoning: Option<String>,
}

impl EvalResult {
    /// Create a passing result with score 1.0.
    pub fn pass() -> Self {
        Self {
            score: 1.0,
            passed: true,
            reasoning: None,
        }
    }

    /// Create a failing result with score 0.0.
    pub fn fail() -> Self {
        Self {
            score: 0.0,
            passed: false,
            reasoning: None,
        }
    }

    /// Create a result with a specific score. Passes if score >= 0.5.
    pub fn with_score(score: f64) -> Self {
        Self {
            score,
            passed: score >= 0.5,
            reasoning: None,
        }
    }

    /// Attach reasoning to this result.
    pub fn with_reasoning(mut self, reasoning: impl Into<String>) -> Self {
        self.reasoning = Some(reasoning.into());
        self
    }
}

/// Trait for evaluating predictions against references.
#[async_trait]
pub trait Evaluator: Send + Sync {
    /// Evaluate a prediction against a reference, given the original input.
    async fn evaluate(
        &self,
        prediction: &str,
        reference: &str,
        input: &str,
    ) -> Result<EvalResult, SynapticError>;
}
