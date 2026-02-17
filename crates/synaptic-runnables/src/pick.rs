use async_trait::async_trait;
use serde_json::Value;
use synaptic_core::{RunnableConfig, SynapseError};

use crate::Runnable;

/// Extracts specified keys from a JSON object input.
pub struct RunnablePick {
    keys: Vec<String>,
}

impl RunnablePick {
    pub fn new(keys: Vec<String>) -> Self {
        Self { keys }
    }
}

#[async_trait]
impl Runnable<Value, Value> for RunnablePick {
    async fn invoke(&self, input: Value, _config: &RunnableConfig) -> Result<Value, SynapseError> {
        let obj = match &input {
            Value::Object(map) => map,
            other => {
                return Err(SynapseError::Validation(format!(
                    "RunnablePick expects a JSON object, got {}",
                    other
                )))
            }
        };

        let mut result = serde_json::Map::new();
        for key in &self.keys {
            if let Some(value) = obj.get(key) {
                result.insert(key.clone(), value.clone());
            }
        }

        Ok(Value::Object(result))
    }
}
