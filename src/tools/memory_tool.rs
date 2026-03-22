//! Agent tools for querying long-term memory.
//!
//! `memory_search` — semantic search over the configured memory provider.
//! `memory_get` — list or count stored memories (LTM-specific).

use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use synaptic::core::{SynapticError, Tool};
use synaptic::memory::MemoryProvider;

use crate::memory::LongTermMemory;

/// Tool that allows the agent to search memory via the configured [`MemoryProvider`].
pub struct MemorySearchTool {
    provider: Arc<dyn MemoryProvider>,
}

#[allow(clippy::new_ret_no_self)]
impl MemorySearchTool {
    pub fn new(provider: Arc<dyn MemoryProvider>) -> Arc<dyn Tool> {
        Arc::new(Self { provider })
    }
}

#[async_trait]
impl Tool for MemorySearchTool {
    fn name(&self) -> &'static str {
        "memory_search"
    }

    fn description(&self) -> &'static str {
        "Search long-term memory for relevant past information. Use this when you need to recall facts, decisions, preferences, or context from previous conversations."
    }

    fn parameters(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "The search query to find relevant memories"
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum number of memories to return (default: 5)",
                    "default": 5
                }
            },
            "required": ["query"]
        }))
    }

    async fn call(&self, args: Value) -> Result<Value, SynapticError> {
        let query = args
            .get("query")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SynapticError::Tool("missing required parameter 'query'".into()))?;

        let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(5) as usize;

        tracing::debug!("memory search");

        let results = self.provider.recall(query, limit).await?;

        if results.is_empty() {
            Ok(json!("No relevant memories found."))
        } else {
            let formatted: Vec<String> = results
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    let layer_tag = r
                        .layer
                        .as_deref()
                        .map(|l| format!(" [{}]", l))
                        .unwrap_or_default();
                    format!("{}. [{}]{} {}", i + 1, r.uri, layer_tag, r.content)
                })
                .collect();
            Ok(json!(formatted.join("\n")))
        }
    }
}

/// Tool that allows the agent to save a memory to long-term storage.
///
/// When the user says "remember this" or the agent wants to persist important
/// information across sessions, this tool writes to LTM.
pub struct MemorySaveTool {
    ltm: Arc<LongTermMemory>,
}

#[allow(clippy::new_ret_no_self)]
impl MemorySaveTool {
    pub fn new(ltm: Arc<LongTermMemory>) -> Arc<dyn Tool> {
        Arc::new(Self { ltm })
    }
}

#[async_trait]
impl Tool for MemorySaveTool {
    fn name(&self) -> &'static str {
        "memory_save"
    }

    fn description(&self) -> &'static str {
        "Save information to long-term memory so it persists across sessions. Use this when the user asks you to remember something, or when you learn important facts, decisions, preferences, or context that should be retained."
    }

    fn parameters(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "content": {
                    "type": "string",
                    "description": "The information to remember. Be concise and factual."
                },
                "evergreen": {
                    "type": "boolean",
                    "description": "If true, this memory is protected from automatic pruning. Use for critical facts like user preferences or important decisions. Default: false.",
                    "default": false
                }
            },
            "required": ["content"]
        }))
    }

    async fn call(&self, args: Value) -> Result<Value, SynapticError> {
        let content = args
            .get("content")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SynapticError::Tool("missing required parameter 'content'".into()))?;

        let evergreen = args
            .get("evergreen")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);

        if evergreen {
            self.ltm.remember_evergreen(content).await?;
        } else {
            self.ltm.remember(content).await?;
        }

        tracing::info!(evergreen, "memory saved via tool");
        Ok(json!(format!(
            "Memory saved{}.",
            if evergreen { " (evergreen)" } else { "" }
        )))
    }
}

/// Tool that allows the agent to forget/delete memories by keyword.
pub struct MemoryForgetTool {
    ltm: Arc<LongTermMemory>,
}

#[allow(clippy::new_ret_no_self)]
impl MemoryForgetTool {
    pub fn new(ltm: Arc<LongTermMemory>) -> Arc<dyn Tool> {
        Arc::new(Self { ltm })
    }
}

#[async_trait]
impl Tool for MemoryForgetTool {
    fn name(&self) -> &'static str {
        "memory_forget"
    }

    fn description(&self) -> &'static str {
        "Delete memories containing a specific keyword. Use when the user asks you to forget something or when stored information is no longer relevant."
    }

    fn parameters(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "keyword": {
                    "type": "string",
                    "description": "Delete all memories containing this keyword (case-insensitive)"
                }
            },
            "required": ["keyword"]
        }))
    }

    async fn call(&self, args: Value) -> Result<Value, SynapticError> {
        let keyword = args
            .get("keyword")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SynapticError::Tool("missing required parameter 'keyword'".into()))?;

        let deleted = self.ltm.forget(keyword).await?;
        tracing::info!(keyword, deleted, "memories forgotten via tool");
        Ok(json!(format!("{} memories deleted.", deleted)))
    }
}

/// Tool that allows the agent to list or count memories.
pub struct MemoryGetTool {
    ltm: Arc<LongTermMemory>,
}

#[allow(clippy::new_ret_no_self)]
impl MemoryGetTool {
    pub fn new(ltm: Arc<LongTermMemory>) -> Arc<dyn Tool> {
        Arc::new(Self { ltm })
    }
}

#[async_trait]
impl Tool for MemoryGetTool {
    fn name(&self) -> &'static str {
        "memory_get"
    }

    fn description(&self) -> &'static str {
        "Get information about stored long-term memories. Use 'list' to see all memories or 'count' to get the total number."
    }

    fn parameters(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "action": {
                    "type": "string",
                    "enum": ["list", "count"],
                    "description": "Action to perform: 'list' returns all memories, 'count' returns the total count"
                }
            },
            "required": ["action"]
        }))
    }

    async fn call(&self, args: Value) -> Result<Value, SynapticError> {
        let action = args
            .get("action")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SynapticError::Tool("missing required parameter 'action'".into()))?;

        match action {
            "count" => {
                let count = self.ltm.count().await;
                Ok(json!(format!("{} memories stored.", count)))
            }
            "list" => {
                let memories = self.ltm.list().await;
                if memories.is_empty() {
                    Ok(json!("No memories stored."))
                } else {
                    let formatted: Vec<String> = memories
                        .iter()
                        .enumerate()
                        .map(|(i, (key, content))| {
                            let preview = if content.len() > 200 {
                                format!("{}...", &content[..197])
                            } else {
                                content.clone()
                            };
                            format!("{}. [{}] {}", i + 1, key, preview)
                        })
                        .collect();
                    Ok(json!(formatted.join("\n")))
                }
            }
            _ => Err(SynapticError::Tool(format!(
                "unknown action '{}', expected 'list' or 'count'",
                action
            ))),
        }
    }
}
