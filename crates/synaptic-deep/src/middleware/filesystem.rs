use async_trait::async_trait;
use serde_json::Value;
use std::sync::Arc;
use synaptic_core::SynapticError;
use synaptic_middleware::{AgentMiddleware, ToolCallRequest, ToolCaller};

use crate::backend::Backend;

/// Middleware that evicts large tool results to files in the backend.
///
/// After a tool call, if the result exceeds `eviction_threshold` tokens (~4 chars/token),
/// the full result is written to `.evicted/{tool_call_id}.txt` and replaced with a
/// preview showing the first and last 5 lines.
pub struct FilesystemMiddleware {
    backend: Arc<dyn Backend>,
    eviction_threshold_chars: usize,
}

impl FilesystemMiddleware {
    pub fn new(backend: Arc<dyn Backend>, eviction_threshold_tokens: usize) -> Self {
        Self {
            backend,
            eviction_threshold_chars: eviction_threshold_tokens * 4,
        }
    }
}

#[async_trait]
impl AgentMiddleware for FilesystemMiddleware {
    async fn wrap_tool_call(
        &self,
        request: ToolCallRequest,
        next: &dyn ToolCaller,
    ) -> Result<Value, SynapticError> {
        let tool_call_id = request.call.id.clone();
        let result = next.call(request).await?;

        let result_str = match &result {
            Value::String(s) => s.clone(),
            other => serde_json::to_string(other).unwrap_or_default(),
        };

        if result_str.len() > self.eviction_threshold_chars {
            let eviction_path = format!(".evicted/{}.txt", tool_call_id);
            let _ = self.backend.write_file(&eviction_path, &result_str).await;

            let lines: Vec<&str> = result_str.lines().collect();
            let preview = if lines.len() <= 10 {
                result_str
            } else {
                let first = &lines[..5];
                let last = &lines[lines.len() - 5..];
                format!(
                    "{}\n\n... ({} lines omitted, full result in {}) ...\n\n{}",
                    first.join("\n"),
                    lines.len() - 10,
                    eviction_path,
                    last.join("\n")
                )
            };
            Ok(Value::String(preview))
        } else {
            Ok(result)
        }
    }
}
