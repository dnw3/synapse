use std::collections::HashMap;

use async_trait::async_trait;
use synaptic_core::{Message, RunnableConfig, SynapseError};
use synaptic_runnables::Runnable;

use crate::PromptTemplate;

/// An example for few-shot prompting, consisting of input/output pairs.
#[derive(Debug, Clone)]
pub struct FewShotExample {
    pub input: String,
    pub output: String,
}

/// A few-shot chat message prompt template that injects examples before the user query.
///
/// Each example is formatted as a Human message (input) followed by an AI message (output).
/// An optional prefix system message can be provided.
pub struct FewShotChatMessagePromptTemplate {
    examples: Vec<FewShotExample>,
    prefix: Option<PromptTemplate>,
    suffix: PromptTemplate,
}

impl FewShotChatMessagePromptTemplate {
    pub fn new(examples: Vec<FewShotExample>, suffix: PromptTemplate) -> Self {
        Self {
            examples,
            prefix: None,
            suffix,
        }
    }

    pub fn with_prefix(mut self, prefix: PromptTemplate) -> Self {
        self.prefix = Some(prefix);
        self
    }

    pub fn format(&self, values: &HashMap<String, String>) -> Result<Vec<Message>, SynapseError> {
        let mut messages = Vec::new();

        if let Some(prefix) = &self.prefix {
            let content = prefix
                .render(values)
                .map_err(|e| SynapseError::Prompt(e.to_string()))?;
            messages.push(Message::system(content));
        }

        for example in &self.examples {
            messages.push(Message::human(&example.input));
            messages.push(Message::ai(&example.output));
        }

        let content = self
            .suffix
            .render(values)
            .map_err(|e| SynapseError::Prompt(e.to_string()))?;
        messages.push(Message::human(content));

        Ok(messages)
    }
}

#[async_trait]
impl Runnable<HashMap<String, String>, Vec<Message>> for FewShotChatMessagePromptTemplate {
    async fn invoke(
        &self,
        input: HashMap<String, String>,
        _config: &RunnableConfig,
    ) -> Result<Vec<Message>, SynapseError> {
        self.format(&input)
    }
}
