use async_trait::async_trait;
use synaptic_core::SynapticError;

use crate::evaluator::{EvalResult, Evaluator};

/// Evaluator that checks whether the prediction is valid JSON.
pub struct JsonValidityEvaluator;

impl JsonValidityEvaluator {
    pub fn new() -> Self {
        Self
    }
}

impl Default for JsonValidityEvaluator {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Evaluator for JsonValidityEvaluator {
    async fn evaluate(
        &self,
        prediction: &str,
        _reference: &str,
        _input: &str,
    ) -> Result<EvalResult, SynapticError> {
        match serde_json::from_str::<serde_json::Value>(prediction) {
            Ok(_) => Ok(EvalResult::pass()),
            Err(e) => Ok(EvalResult::fail().with_reasoning(format!("Invalid JSON: {}", e))),
        }
    }
}
