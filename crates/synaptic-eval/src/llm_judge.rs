use std::sync::Arc;

use async_trait::async_trait;
use synaptic_core::{ChatModel, ChatRequest, Message, SynapticError};

use crate::evaluator::{EvalResult, Evaluator};

const DEFAULT_PROMPT_TEMPLATE: &str = r#"You are an impartial judge evaluating the quality of an AI response.

Input: {input}
Expected answer: {reference}
AI response: {prediction}

Rate the AI response on a scale of 0 to 10, where 0 means completely wrong and 10 means perfect.
Respond with ONLY a single integer between 0 and 10."#;

/// Evaluator that uses an LLM to judge prediction quality.
pub struct LLMJudgeEvaluator {
    model: Arc<dyn ChatModel>,
    prompt_template: String,
}

impl LLMJudgeEvaluator {
    /// Create a new LLM judge evaluator with the default prompt template.
    pub fn new(model: Arc<dyn ChatModel>) -> Self {
        Self {
            model,
            prompt_template: DEFAULT_PROMPT_TEMPLATE.to_string(),
        }
    }

    /// Create a new LLM judge evaluator with a custom prompt template.
    ///
    /// The template should contain `{input}`, `{prediction}`, and `{reference}` placeholders.
    pub fn with_prompt(model: Arc<dyn ChatModel>, template: impl Into<String>) -> Self {
        Self {
            model,
            prompt_template: template.into(),
        }
    }
}

/// Parse a score (0-10) from the model's response text.
fn parse_score(text: &str) -> Option<f64> {
    // Look for a number in the response
    for word in text.split_whitespace() {
        let cleaned = word.trim_matches(|c: char| !c.is_ascii_digit() && c != '.');
        if let Ok(num) = cleaned.parse::<f64>() {
            if (0.0..=10.0).contains(&num) {
                return Some(num / 10.0);
            }
        }
    }
    None
}

#[async_trait]
impl Evaluator for LLMJudgeEvaluator {
    async fn evaluate(
        &self,
        prediction: &str,
        reference: &str,
        input: &str,
    ) -> Result<EvalResult, SynapticError> {
        let prompt = self
            .prompt_template
            .replace("{input}", input)
            .replace("{prediction}", prediction)
            .replace("{reference}", reference);

        let request = ChatRequest::new(vec![Message::human(prompt)]);
        let response = self.model.chat(request).await?;
        let response_text = response.message.content();

        match parse_score(response_text) {
            Some(score) => Ok(EvalResult::with_score(score)
                .with_reasoning(format!("LLM judge score: {:.1}/10", score * 10.0))),
            None => Err(SynapticError::Parsing(format!(
                "Could not parse score from LLM response: {:?}",
                response_text
            ))),
        }
    }
}
