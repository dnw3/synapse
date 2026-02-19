use async_trait::async_trait;
use serde_json::Value;
use synaptic_core::{RunnableConfig, SynapticError};

use crate::runnable::{BoxRunnable, Runnable};

/// Runs named branches in parallel on the input, then merges results into the input object.
/// Input must be a JSON object. Each branch receives a clone of the full input.
pub struct RunnableAssign {
    branches: Vec<(String, BoxRunnable<Value, Value>)>,
}

impl RunnableAssign {
    pub fn new(branches: Vec<(String, BoxRunnable<Value, Value>)>) -> Self {
        Self { branches }
    }
}

#[async_trait]
impl Runnable<Value, Value> for RunnableAssign {
    async fn invoke(&self, input: Value, config: &RunnableConfig) -> Result<Value, SynapticError> {
        let mut base = match input {
            Value::Object(map) => map,
            other => {
                return Err(SynapticError::Validation(format!(
                    "RunnableAssign expects a JSON object, got {}",
                    other
                )))
            }
        };

        let futures: Vec<_> = self
            .branches
            .iter()
            .map(|(key, runnable)| {
                let input_clone = Value::Object(base.clone());
                let key = key.clone();
                async move {
                    let result = runnable.invoke(input_clone, config).await?;
                    Ok::<_, SynapticError>((key, result))
                }
            })
            .collect();

        let results = futures::future::join_all(futures).await;
        for result in results {
            let (key, value) = result?;
            base.insert(key, value);
        }

        Ok(Value::Object(base))
    }
}
