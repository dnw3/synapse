use async_trait::async_trait;
use synaptic_core::SynapseError;

use crate::evaluator::{EvalResult, Evaluator};

/// Evaluator that checks for exact string match between prediction and reference.
pub struct ExactMatchEvaluator {
    ignore_case: bool,
}

impl ExactMatchEvaluator {
    /// Create a case-sensitive exact match evaluator.
    pub fn new() -> Self {
        Self { ignore_case: false }
    }

    /// Create a case-insensitive exact match evaluator.
    pub fn case_insensitive() -> Self {
        Self { ignore_case: true }
    }
}

impl Default for ExactMatchEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Evaluator for ExactMatchEvaluator {
    async fn evaluate(
        &self,
        prediction: &str,
        reference: &str,
        _input: &str,
    ) -> Result<EvalResult, SynapseError> {
        let matches = if self.ignore_case {
            prediction.to_lowercase() == reference.to_lowercase()
        } else {
            prediction == reference
        };

        if matches {
            Ok(EvalResult::pass())
        } else {
            Ok(EvalResult::fail()
                .with_reasoning(format!("Expected {:?}, got {:?}", reference, prediction)))
        }
    }
}
