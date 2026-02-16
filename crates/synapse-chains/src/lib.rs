use async_trait::async_trait;
use synapse_core::{RunnableConfig, SynapseError};
use synapse_runnables::{BoxRunnable, Runnable};

pub struct SequentialChain {
    steps: Vec<BoxRunnable<String, String>>,
}

impl SequentialChain {
    pub fn new(steps: Vec<BoxRunnable<String, String>>) -> Self {
        Self { steps }
    }
}

#[async_trait]
impl Runnable<String, String> for SequentialChain {
    async fn invoke(&self, input: String, config: &RunnableConfig) -> Result<String, SynapseError> {
        let mut current = input;
        for step in &self.steps {
            current = step.invoke(current, config).await?;
        }
        Ok(current)
    }
}
