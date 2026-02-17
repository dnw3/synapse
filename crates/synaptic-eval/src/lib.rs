mod dataset;
mod embedding_distance;
mod evaluator;
mod exact_match;
mod json_validity;
mod llm_judge;
mod regex_match;

pub use dataset::{evaluate, Dataset, DatasetItem};
pub use embedding_distance::EmbeddingDistanceEvaluator;
pub use evaluator::{EvalResult, Evaluator};
pub use exact_match::ExactMatchEvaluator;
pub use json_validity::JsonValidityEvaluator;
pub use llm_judge::LLMJudgeEvaluator;
pub use regex_match::RegexMatchEvaluator;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvalCase {
    pub expected: String,
    pub actual: String,
}

impl EvalCase {
    pub fn new(expected: impl Into<String>, actual: impl Into<String>) -> Self {
        Self {
            expected: expected.into(),
            actual: actual.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct EvalReport {
    pub total: usize,
    pub passed: usize,
    pub accuracy: f32,
    pub results: Vec<EvalResult>,
}

impl EvalReport {
    /// Create a report from legacy `EvalCase` values (results will be empty).
    pub fn from_cases(cases: Vec<EvalCase>) -> Self {
        let total = cases.len();
        let passed = cases
            .iter()
            .filter(|case| case.expected == case.actual)
            .count();
        let accuracy = if total == 0 {
            0.0
        } else {
            passed as f32 / total as f32
        };
        Self {
            total,
            passed,
            accuracy,
            results: Vec::new(),
        }
    }

    /// Create a report from evaluator results.
    pub fn from_results(results: Vec<EvalResult>) -> Self {
        let total = results.len();
        let passed = results.iter().filter(|r| r.passed).count();
        let accuracy = if total == 0 {
            0.0
        } else {
            passed as f32 / total as f32
        };
        Self {
            total,
            passed,
            accuracy,
            results,
        }
    }
}
