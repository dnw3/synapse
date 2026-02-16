use async_trait::async_trait;
use serde_json::Value;
use synapse_core::{RunnableConfig, SynapseError};

use crate::runnable::{BoxRunnable, Runnable};

/// Runs multiple named runnables concurrently on the same (cloned) input,
/// merging outputs into a JSON object keyed by branch name.
pub struct RunnableParallel<I: Send + Clone + 'static> {
    branches: Vec<(String, BoxRunnable<I, Value>)>,
}

impl<I: Send + Clone + 'static> RunnableParallel<I> {
    pub fn new(branches: Vec<(String, BoxRunnable<I, Value>)>) -> Self {
        Self { branches }
    }
}

#[async_trait]
impl<I: Send + Clone + 'static> Runnable<I, Value> for RunnableParallel<I> {
    async fn invoke(&self, input: I, config: &RunnableConfig) -> Result<Value, SynapseError> {
        let futures: Vec<_> = self
            .branches
            .iter()
            .map(|(key, runnable)| {
                let input_clone = input.clone();
                let key = key.clone();
                async move {
                    let result = runnable.invoke(input_clone, config).await?;
                    Ok::<_, SynapseError>((key, result))
                }
            })
            .collect();

        let results = futures::future::join_all(futures).await;
        let mut map = serde_json::Map::new();
        for result in results {
            let (key, value) = result?;
            map.insert(key, value);
        }
        Ok(Value::Object(map))
    }
}
