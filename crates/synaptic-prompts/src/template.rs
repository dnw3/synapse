use std::collections::HashMap;

use thiserror::Error;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PromptError {
    #[error("missing variable: {0}")]
    MissingVariable(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromptTemplate {
    template: String,
    partial_variables: HashMap<String, String>,
}

impl PromptTemplate {
    pub fn new(template: impl Into<String>) -> Self {
        Self {
            template: template.into(),
            partial_variables: HashMap::new(),
        }
    }

    /// Set a partial variable that will be used as a default during rendering.
    /// Provided values at render time take precedence over partial variables.
    pub fn with_partial(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.partial_variables.insert(key.into(), value.into());
        self
    }

    pub fn render(&self, values: &HashMap<String, String>) -> Result<String, PromptError> {
        // Merge partial_variables with provided values; provided values take precedence
        let mut merged = self.partial_variables.clone();
        for (k, v) in values {
            merged.insert(k.clone(), v.clone());
        }

        let mut output = String::with_capacity(self.template.len());
        let mut rest = self.template.as_str();

        while let Some(start) = rest.find("{{") {
            output.push_str(&rest[..start]);
            let after_start = &rest[start + 2..];
            if let Some(end) = after_start.find("}}") {
                let key = after_start[..end].trim();
                let value = merged
                    .get(key)
                    .ok_or_else(|| PromptError::MissingVariable(key.to_string()))?;
                output.push_str(value);
                rest = &after_start[end + 2..];
            } else {
                output.push_str(&rest[start..]);
                rest = "";
                break;
            }
        }

        output.push_str(rest);
        Ok(output)
    }
}
