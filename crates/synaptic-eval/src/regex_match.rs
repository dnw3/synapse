use async_trait::async_trait;
use regex::Regex;
use synaptic_core::SynapseError;

use crate::evaluator::{EvalResult, Evaluator};

/// Evaluator that checks whether the prediction matches a regex pattern.
pub struct RegexMatchEvaluator {
    pattern: Regex,
}

impl RegexMatchEvaluator {
    /// Create a new regex match evaluator with the given pattern.
    ///
    /// Returns an error if the pattern is not a valid regex.
    pub fn new(pattern: &str) -> Result<Self, SynapseError> {
        let pattern = Regex::new(pattern)
            .map_err(|e| SynapseError::Validation(format!("Invalid regex pattern: {}", e)))?;
        Ok(Self { pattern })
    }
}

#[async_trait]
impl Evaluator for RegexMatchEvaluator {
    async fn evaluate(
        &self,
        prediction: &str,
        _reference: &str,
        _input: &str,
    ) -> Result<EvalResult, SynapseError> {
        if self.pattern.is_match(prediction) {
            Ok(EvalResult::pass())
        } else {
            Ok(EvalResult::fail().with_reasoning(format!(
                "Prediction {:?} does not match pattern {:?}",
                prediction,
                self.pattern.as_str()
            )))
        }
    }
}
