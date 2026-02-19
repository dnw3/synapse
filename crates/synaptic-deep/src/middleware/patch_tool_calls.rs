use async_trait::async_trait;
use std::collections::HashSet;
use synaptic_core::{Message, SynapticError};
use synaptic_middleware::{AgentMiddleware, ModelRequest, ModelResponse};

/// Middleware that fixes malformed tool calls in model responses.
///
/// Patches applied:
/// - Strip markdown code fences from JSON arguments
/// - Attempt to parse string arguments as JSON
/// - Deduplicate tool call IDs
/// - Remove tool calls with empty names
pub struct PatchToolCallsMiddleware;

#[async_trait]
impl AgentMiddleware for PatchToolCallsMiddleware {
    async fn after_model(
        &self,
        _request: &ModelRequest,
        response: &mut ModelResponse,
    ) -> Result<(), SynapticError> {
        let tool_calls = response.message.tool_calls().to_vec();
        if tool_calls.is_empty() {
            return Ok(());
        }

        let mut seen_ids = HashSet::new();
        let mut patched = Vec::new();
        let mut id_counter = 0u32;
        let mut changed = false;

        for mut tc in tool_calls {
            // Skip empty names
            if tc.name.trim().is_empty() {
                changed = true;
                continue;
            }

            // Fix JSON arguments
            let fixed_args = fix_json_arguments(&tc.arguments);
            if fixed_args != tc.arguments {
                tc.arguments = fixed_args;
                changed = true;
            }

            // Deduplicate IDs
            if seen_ids.contains(&tc.id) || tc.id.is_empty() {
                tc.id = format!("patched_{}", id_counter);
                id_counter += 1;
                changed = true;
            }
            seen_ids.insert(tc.id.clone());

            patched.push(tc);
        }

        if changed {
            let content = response.message.content().to_string();
            let id = response.message.id().map(|s| s.to_string());
            let mut new_msg = Message::ai_with_tool_calls(content, patched);
            if let Some(id) = id {
                new_msg = new_msg.with_id(id);
            }
            response.message = new_msg;
        }

        Ok(())
    }
}

fn fix_json_arguments(args: &serde_json::Value) -> serde_json::Value {
    if let serde_json::Value::String(s) = args {
        let trimmed = s.trim();
        // Strip markdown code fences
        let cleaned = if trimmed.starts_with("```") {
            let without_start = trimmed
                .trim_start_matches("```json")
                .trim_start_matches("```");
            without_start.trim_end_matches("```").trim()
        } else {
            trimmed
        };

        // Try to parse as JSON
        match serde_json::from_str(cleaned) {
            Ok(v) => v,
            Err(_) => args.clone(),
        }
    } else {
        args.clone()
    }
}
