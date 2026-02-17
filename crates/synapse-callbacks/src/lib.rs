mod composite;
mod stdout;
mod tracing_cb;

pub use composite::CompositeCallback;
pub use stdout::StdOutCallbackHandler;
pub use tracing_cb::TracingCallback;

use std::sync::Arc;

use async_trait::async_trait;
use synapse_core::{CallbackHandler, RunEvent, SynapseError};
use tokio::sync::RwLock;

#[derive(Default, Clone)]
pub struct RecordingCallback {
    events: Arc<RwLock<Vec<RunEvent>>>,
}

impl RecordingCallback {
    pub fn new() -> Self {
        Self::default()
    }

    pub async fn events(&self) -> Vec<RunEvent> {
        self.events.read().await.clone()
    }
}

#[async_trait]
impl CallbackHandler for RecordingCallback {
    async fn on_event(&self, event: RunEvent) -> Result<(), SynapseError> {
        self.events.write().await.push(event);
        Ok(())
    }
}
