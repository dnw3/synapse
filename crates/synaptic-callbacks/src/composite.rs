use std::sync::Arc;

use async_trait::async_trait;
use synaptic_core::{CallbackHandler, RunEvent, SynapticError};

pub struct CompositeCallback {
    handlers: Vec<Arc<dyn CallbackHandler>>,
}

impl CompositeCallback {
    pub fn new(handlers: Vec<Arc<dyn CallbackHandler>>) -> Self {
        Self { handlers }
    }
}

#[async_trait]
impl CallbackHandler for CompositeCallback {
    async fn on_event(&self, event: RunEvent) -> Result<(), SynapticError> {
        for handler in &self.handlers {
            handler.on_event(event.clone()).await?;
        }
        Ok(())
    }
}
