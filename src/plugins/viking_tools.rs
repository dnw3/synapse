//! Viking-specific tools: progressive content loading and active memory commit.
//!
//! These tools use `Arc<VikingMemoryProvider>` directly (not the trait)
//! because they access Viking-specific APIs (L0/L1/L2, session commit).

use std::sync::Arc;

use async_trait::async_trait;
use synaptic::core::{SynapticError, Tool};
use synaptic::memory::MemoryProvider;

use crate::memory::VikingMemoryProvider;

// ---------------------------------------------------------------------------
// VikingContentTool — L0/L1/L2 progressive content loading
// ---------------------------------------------------------------------------

/// Tool for reading Viking memory content at different detail levels.
///
/// Agent workflow: `memory_search` → get URIs → `memory_read` to drill into results.
pub struct VikingContentTool {
    provider: Arc<VikingMemoryProvider>,
}

#[allow(clippy::new_ret_no_self)]
impl VikingContentTool {
    pub fn new(provider: Arc<VikingMemoryProvider>) -> Arc<dyn Tool> {
        Arc::new(Self { provider })
    }
}

#[async_trait]
impl Tool for VikingContentTool {
    fn name(&self) -> &'static str {
        "memory_read"
    }

    fn description(&self) -> &'static str {
        "Read memory content at different detail levels. Use 'abstract' for quick summaries \
         (~100 tokens), 'overview' for navigation (~2k tokens), 'full' for complete content. \
         Use after memory_search to get more detail on specific results."
    }

    fn parameters(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "uri": {
                    "type": "string",
                    "description": "Viking URI from memory_search results (e.g., viking://user/memories/preferences/dark-mode.md)"
                },
                "level": {
                    "type": "string",
                    "enum": ["abstract", "overview", "full"],
                    "description": "Detail level: abstract (~100 tokens), overview (~2k tokens), full (complete content)",
                    "default": "overview"
                }
            },
            "required": ["uri"]
        }))
    }

    async fn call(&self, args: serde_json::Value) -> Result<serde_json::Value, SynapticError> {
        let uri = args["uri"]
            .as_str()
            .ok_or_else(|| SynapticError::Tool("missing required parameter: uri".into()))?;
        let level = args["level"].as_str().unwrap_or("overview");

        self.provider.read_content(uri, level).await
    }
}

// ---------------------------------------------------------------------------
// VikingCommitMemoryTool — active memory write
// ---------------------------------------------------------------------------

/// Tool for the agent to actively save memories to Viking.
///
/// When user says "remember that I like dark mode", the agent calls this tool.
/// The content is added as a user message with save-intent framing,
/// then immediately committed to trigger memory extraction.
pub struct VikingCommitMemoryTool {
    provider: Arc<VikingMemoryProvider>,
}

#[allow(clippy::new_ret_no_self)]
impl VikingCommitMemoryTool {
    pub fn new(provider: Arc<VikingMemoryProvider>) -> Arc<dyn Tool> {
        Arc::new(Self { provider })
    }
}

#[async_trait]
impl Tool for VikingCommitMemoryTool {
    fn name(&self) -> &'static str {
        "memory_commit"
    }

    fn description(&self) -> &'static str {
        "Actively save important information to long-term memory. Use when the user asks \
         you to remember something, or when you encounter information that should be \
         preserved across sessions. The information will be extracted and stored permanently."
    }

    fn parameters(&self) -> Option<serde_json::Value> {
        Some(serde_json::json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "The information to remember"
                },
                "session_key": {
                    "type": "string",
                    "description": "Session identifier (defaults to 'default')",
                    "default": "default"
                }
            },
            "required": ["content"]
        }))
    }

    async fn call(&self, args: serde_json::Value) -> Result<serde_json::Value, SynapticError> {
        let content = args["content"]
            .as_str()
            .ok_or_else(|| SynapticError::Tool("missing required parameter: content".into()))?;
        let session_key = args["session_key"].as_str().unwrap_or("default");

        // Frame as user instruction so Viking's extraction prioritizes it
        let msg = format!("Please remember the following: {content}");
        self.provider.add_message(session_key, "user", &msg).await?;

        // Immediately commit to trigger memory extraction
        let result = self.provider.commit(session_key).await?;

        Ok(serde_json::json!({
            "status": "saved",
            "content": content,
            "memories_extracted": result.memories_extracted,
            "memories_merged": result.memories_merged,
        }))
    }
}
