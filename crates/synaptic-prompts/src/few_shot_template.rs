use std::collections::HashMap;

use async_trait::async_trait;
use synaptic_core::{RunnableConfig, SynapseError};
use synaptic_runnables::Runnable;

use crate::{FewShotExample, PromptTemplate};

/// A string-based few-shot prompt template (as opposed to `FewShotChatMessagePromptTemplate`
/// which produces `Vec<Message>`).
///
/// Produces a single formatted string with examples embedded.
pub struct FewShotPromptTemplate {
    examples: Vec<FewShotExample>,
    example_prompt: PromptTemplate,
    prefix: Option<String>,
    suffix: PromptTemplate,
    example_separator: String,
}

impl FewShotPromptTemplate {
    /// Create a new string-based few-shot template.
    ///
    /// - `examples`: the input/output example pairs
    /// - `example_prompt`: template for each example, e.g. `"Input: {{ input }}\nOutput: {{ output }}"`
    /// - `suffix`: final template rendered with user-provided variables
    pub fn new(
        examples: Vec<FewShotExample>,
        example_prompt: PromptTemplate,
        suffix: PromptTemplate,
    ) -> Self {
        Self {
            examples,
            example_prompt,
            prefix: None,
            suffix,
            example_separator: "\n\n".to_string(),
        }
    }

    /// Set an optional prefix string prepended before the examples.
    pub fn with_prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    /// Set the separator used between examples (default: `"\n\n"`).
    pub fn with_separator(mut self, sep: impl Into<String>) -> Self {
        self.example_separator = sep.into();
        self
    }

    /// Render the template with the given variable values.
    ///
    /// 1. Start with prefix (if any)
    /// 2. For each example, render `example_prompt` with `{"input": ..., "output": ...}`
    /// 3. Join examples with separator
    /// 4. Append suffix rendered with provided values
    /// 5. Join all parts with separator
    pub fn render(&self, values: &HashMap<String, String>) -> Result<String, SynapseError> {
        let mut parts: Vec<String> = Vec::new();

        // 1. Prefix
        if let Some(prefix) = &self.prefix {
            parts.push(prefix.clone());
        }

        // 2-3. Render and join examples
        let mut example_strings = Vec::with_capacity(self.examples.len());
        for example in &self.examples {
            let example_values = HashMap::from([
                ("input".to_string(), example.input.clone()),
                ("output".to_string(), example.output.clone()),
            ]);
            let rendered = self
                .example_prompt
                .render(&example_values)
                .map_err(|e| SynapseError::Prompt(e.to_string()))?;
            example_strings.push(rendered);
        }

        if !example_strings.is_empty() {
            parts.push(example_strings.join(&self.example_separator));
        }

        // 4. Suffix
        let suffix_rendered = self
            .suffix
            .render(values)
            .map_err(|e| SynapseError::Prompt(e.to_string()))?;
        parts.push(suffix_rendered);

        // 5. Join all parts
        Ok(parts.join(&self.example_separator))
    }
}

#[async_trait]
impl Runnable<HashMap<String, String>, String> for FewShotPromptTemplate {
    async fn invoke(
        &self,
        input: HashMap<String, String>,
        _config: &RunnableConfig,
    ) -> Result<String, SynapseError> {
        self.render(&input)
    }
}
