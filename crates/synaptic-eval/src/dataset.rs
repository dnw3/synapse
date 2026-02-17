use synaptic_core::SynapseError;

use crate::evaluator::Evaluator;
use crate::EvalReport;

/// A single item in an evaluation dataset.
#[derive(Debug, Clone)]
pub struct DatasetItem {
    pub input: String,
    pub reference: String,
}

/// A collection of input-reference pairs for evaluation.
#[derive(Debug, Clone)]
pub struct Dataset {
    pub items: Vec<DatasetItem>,
}

impl Dataset {
    /// Create a new dataset from items.
    pub fn new(items: Vec<DatasetItem>) -> Self {
        Self { items }
    }

    /// Create a dataset from (input, reference) string pairs.
    pub fn from_pairs(pairs: Vec<(&str, &str)>) -> Self {
        Self {
            items: pairs
                .into_iter()
                .map(|(i, r)| DatasetItem {
                    input: i.to_string(),
                    reference: r.to_string(),
                })
                .collect(),
        }
    }
}

/// Evaluate predictions against a dataset using an evaluator.
///
/// Each prediction is evaluated against the corresponding dataset item.
/// The number of predictions must match the number of dataset items.
pub async fn evaluate(
    evaluator: &dyn Evaluator,
    dataset: &Dataset,
    predictions: &[String],
) -> Result<EvalReport, SynapseError> {
    if predictions.len() != dataset.items.len() {
        return Err(SynapseError::Validation(format!(
            "Number of predictions ({}) does not match dataset size ({})",
            predictions.len(),
            dataset.items.len()
        )));
    }

    let mut results = Vec::with_capacity(dataset.items.len());

    for (prediction, item) in predictions.iter().zip(dataset.items.iter()) {
        let result = evaluator
            .evaluate(prediction, &item.reference, &item.input)
            .await?;
        results.push(result);
    }

    Ok(EvalReport::from_results(results))
}
