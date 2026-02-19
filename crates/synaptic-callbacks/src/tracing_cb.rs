use async_trait::async_trait;
use synaptic_core::{CallbackHandler, RunEvent, SynapticError};

pub struct TracingCallback;

impl TracingCallback {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TracingCallback {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CallbackHandler for TracingCallback {
    async fn on_event(&self, event: RunEvent) -> Result<(), SynapticError> {
        match event {
            RunEvent::RunStarted { run_id, session_id } => {
                tracing::info!(run_id = %run_id, session_id = %session_id, "run started");
            }
            RunEvent::RunStep { run_id, step } => {
                tracing::info!(run_id = %run_id, step = step, "run step");
            }
            RunEvent::LlmCalled {
                run_id,
                message_count,
            } => {
                tracing::info!(run_id = %run_id, message_count = message_count, "LLM called");
            }
            RunEvent::ToolCalled { run_id, tool_name } => {
                tracing::info!(run_id = %run_id, tool_name = %tool_name, "tool called");
            }
            RunEvent::RunFinished { run_id, output } => {
                tracing::info!(run_id = %run_id, output_len = output.len(), "run finished");
            }
            RunEvent::RunFailed { run_id, error } => {
                tracing::error!(run_id = %run_id, error = %error, "run failed");
            }
        }
        Ok(())
    }
}
