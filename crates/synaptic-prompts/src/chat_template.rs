use std::collections::HashMap;

use async_trait::async_trait;
use serde_json::Value;
use synaptic_core::{Message, RunnableConfig, SynapticError};
use synaptic_runnables::Runnable;

use crate::PromptTemplate;

/// A template component that produces one or more Messages.
pub enum MessageTemplate {
    /// Renders a system message from a template string.
    System(PromptTemplate),
    /// Renders a human message from a template string.
    Human(PromptTemplate),
    /// Renders an AI message from a template string.
    AI(PromptTemplate),
    /// Injects messages from the input map under the given key.
    /// The value at that key must be a JSON array of Message objects.
    Placeholder(String),
}

/// A chat prompt template that renders a sequence of messages.
///
/// ```ignore
/// let prompt = ChatPromptTemplate::from_messages(vec![
///     MessageTemplate::System(PromptTemplate::new("You are a helpful assistant.")),
///     MessageTemplate::Placeholder("history".to_string()),
///     MessageTemplate::Human(PromptTemplate::new("{{ input }}")),
/// ]);
/// ```
pub struct ChatPromptTemplate {
    templates: Vec<MessageTemplate>,
}

impl ChatPromptTemplate {
    pub fn new(templates: Vec<MessageTemplate>) -> Self {
        Self { templates }
    }

    /// Alias for `new`, matching LangChain's factory method name.
    pub fn from_messages(templates: Vec<MessageTemplate>) -> Self {
        Self::new(templates)
    }

    /// Render the templates against the given variables, producing a list of messages.
    pub fn format(&self, values: &HashMap<String, Value>) -> Result<Vec<Message>, SynapticError> {
        let mut messages = Vec::new();

        // Build a string map for PromptTemplate rendering
        let string_values: HashMap<String, String> = values
            .iter()
            .filter_map(|(k, v)| {
                if let Value::String(s) = v {
                    Some((k.clone(), s.clone()))
                } else {
                    None
                }
            })
            .collect();

        for template in &self.templates {
            match template {
                MessageTemplate::System(pt) => {
                    let content = pt
                        .render(&string_values)
                        .map_err(|e| SynapticError::Prompt(e.to_string()))?;
                    messages.push(Message::system(content));
                }
                MessageTemplate::Human(pt) => {
                    let content = pt
                        .render(&string_values)
                        .map_err(|e| SynapticError::Prompt(e.to_string()))?;
                    messages.push(Message::human(content));
                }
                MessageTemplate::AI(pt) => {
                    let content = pt
                        .render(&string_values)
                        .map_err(|e| SynapticError::Prompt(e.to_string()))?;
                    messages.push(Message::ai(content));
                }
                MessageTemplate::Placeholder(key) => {
                    let value = values.get(key).ok_or_else(|| {
                        SynapticError::Prompt(format!("missing placeholder: {key}"))
                    })?;
                    let msgs: Vec<Message> =
                        serde_json::from_value(value.clone()).map_err(|e| {
                            SynapticError::Prompt(format!(
                                "invalid messages for placeholder '{key}': {e}"
                            ))
                        })?;
                    messages.extend(msgs);
                }
            }
        }

        Ok(messages)
    }
}

#[async_trait]
impl Runnable<HashMap<String, Value>, Vec<Message>> for ChatPromptTemplate {
    async fn invoke(
        &self,
        input: HashMap<String, Value>,
        _config: &RunnableConfig,
    ) -> Result<Vec<Message>, SynapticError> {
        self.format(&input)
    }
}
