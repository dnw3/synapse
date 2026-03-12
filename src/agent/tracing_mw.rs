use async_trait::async_trait;
use serde_json::Value;
use synaptic::core::SynapticError;
use synaptic::middleware::{
    AgentMiddleware, ModelCaller, ModelRequest, ModelResponse, ToolCallRequest, ToolCaller,
};

/// Logs model call and tool call metrics with full content and latency tracking.
pub struct AgentTracingMiddleware;

impl AgentTracingMiddleware {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl AgentMiddleware for AgentTracingMiddleware {
    /// Wraps the entire model call to measure end-to-end latency.
    async fn wrap_model_call(
        &self,
        request: ModelRequest,
        next: &dyn ModelCaller,
    ) -> Result<ModelResponse, SynapticError> {
        let message_count = request.messages.len();
        let tool_count = request.tools.len();
        let has_thinking = request.thinking.is_some();

        let system_prompt_len = request.system_prompt.as_ref().map(|s| s.len()).unwrap_or(0);
        let system_prompt = request.system_prompt.clone().unwrap_or_default();
        let user_message = request
            .messages
            .iter()
            .rev()
            .find(|m| m.is_human())
            .map(|m| m.content().to_string())
            .unwrap_or_default();

        tracing::info!(
            message_count,
            tool_count,
            has_thinking,
            system_prompt_len,
            system_prompt = %system_prompt,
            user_message = %user_message,
            "model call starting"
        );

        let start = std::time::Instant::now();
        let result = next.call(request).await;
        let duration_ms = start.elapsed().as_millis() as u64;

        match &result {
            Ok(response) => {
                let tool_calls_count = response.message.tool_calls().len();
                let content = response.message.content().to_string();

                let tool_names: Vec<String> = response
                    .message
                    .tool_calls()
                    .iter()
                    .map(|tc| format!("{}({})", tc.name, tc.arguments))
                    .collect();
                let tools_summary = tool_names.join(", ");

                if let Some(ref usage) = response.usage {
                    tracing::info!(
                        duration_ms,
                        input_tokens = usage.input_tokens,
                        output_tokens = usage.output_tokens,
                        total_tokens = usage.total_tokens,
                        tool_calls = tool_calls_count,
                        tools = %tools_summary,
                        response = %content,
                        "model call completed"
                    );
                } else {
                    tracing::info!(
                        duration_ms,
                        tool_calls = tool_calls_count,
                        tools = %tools_summary,
                        response = %content,
                        "model call completed (no usage)"
                    );
                }
            }
            Err(e) => {
                tracing::error!(
                    duration_ms,
                    error = %e,
                    "model call failed"
                );
            }
        }

        result
    }

    /// Wraps each tool call to measure latency and log errors.
    async fn wrap_tool_call(
        &self,
        request: ToolCallRequest,
        next: &dyn ToolCaller,
    ) -> Result<Value, SynapticError> {
        let tool_name = request.call.name.clone();
        let args = request.call.arguments.to_string();

        tracing::info!(tool = %tool_name, args = %args, "tool call starting");

        let start = std::time::Instant::now();
        let result = next.call(request).await;
        let duration_ms = start.elapsed().as_millis() as u64;

        match &result {
            Ok(val) => {
                let result_str = val.to_string();
                tracing::info!(
                    tool = %tool_name,
                    duration_ms,
                    result = %result_str,
                    "tool call completed"
                );
            }
            Err(e) => {
                tracing::error!(
                    tool = %tool_name,
                    duration_ms,
                    error = %e,
                    "tool call failed"
                );
            }
        }

        result
    }
}
