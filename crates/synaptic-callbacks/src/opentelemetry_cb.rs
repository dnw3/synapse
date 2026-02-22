//! OpenTelemetry callback handler for Synaptic.
//!
//! Emits spans to any OTel-compatible backend via the global OTel tracer.

use async_trait::async_trait;
use opentelemetry::{
    global,
    trace::{Span, Tracer},
    KeyValue,
};
use synaptic_core::{CallbackHandler, RunEvent, SynapticError};

/// Callback handler that records Synaptic run events as OpenTelemetry spans.
///
/// Each LLM call and tool invocation creates a brief span via the global OTel tracer.
pub struct OpenTelemetryCallback {
    service_name: String,
}

impl OpenTelemetryCallback {
    /// Create a new OpenTelemetry callback with the given service name.
    ///
    /// Uses the global OTel tracer. Set up your OTel provider before calling.
    pub fn new(service_name: impl Into<String>) -> Self {
        Self {
            service_name: service_name.into(),
        }
    }
}

#[async_trait]
impl CallbackHandler for OpenTelemetryCallback {
    async fn on_event(&self, event: RunEvent) -> Result<(), SynapticError> {
        let tracer = global::tracer(self.service_name.clone());
        match &event {
            RunEvent::RunStarted { run_id, .. } => {
                let mut span = tracer
                    .span_builder("synaptic.run_started")
                    .with_attributes(vec![KeyValue::new("synaptic.run_id", run_id.to_string())])
                    .start(&tracer);
                span.end();
            }
            RunEvent::LlmCalled {
                run_id,
                message_count,
            } => {
                let mut span = tracer
                    .span_builder("synaptic.llm_called")
                    .with_attributes(vec![
                        KeyValue::new("synaptic.run_id", run_id.to_string()),
                        KeyValue::new("llm.message_count", *message_count as i64),
                    ])
                    .start(&tracer);
                span.end();
            }
            RunEvent::ToolCalled { run_id, tool_name } => {
                let mut span = tracer
                    .span_builder(format!("tool.{}", tool_name))
                    .with_attributes(vec![
                        KeyValue::new("synaptic.run_id", run_id.to_string()),
                        KeyValue::new("tool.name", tool_name.clone()),
                    ])
                    .start(&tracer);
                span.end();
            }
            RunEvent::RunStep { run_id, step } => {
                let mut span = tracer
                    .span_builder("synaptic.run_step")
                    .with_attributes(vec![
                        KeyValue::new("synaptic.run_id", run_id.to_string()),
                        KeyValue::new("synaptic.step", *step as i64),
                    ])
                    .start(&tracer);
                span.end();
            }
            RunEvent::RunFinished { run_id, .. } => {
                let mut span = tracer
                    .span_builder("synaptic.run_finished")
                    .with_attributes(vec![KeyValue::new("synaptic.run_id", run_id.to_string())])
                    .start(&tracer);
                span.end();
            }
            RunEvent::RunFailed { run_id, error } => {
                let mut span = tracer
                    .span_builder("synaptic.run_failed")
                    .with_attributes(vec![
                        KeyValue::new("synaptic.run_id", run_id.to_string()),
                        KeyValue::new("error.message", error.clone()),
                    ])
                    .start(&tracer);
                span.end();
            }
        }
        Ok(())
    }
}
