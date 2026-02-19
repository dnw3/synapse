//! Deep agent harness for Synaptic.
//!
//! Provides an opinionated agent harness that bundles filesystem tools,
//! subagent spawning, skills, memory, and auto-summarization — all
//! implemented as [`AgentMiddleware`](synaptic_middleware::AgentMiddleware).
//!
//! # Quick Start
//!
//! ```rust,ignore
//! use synaptic_deep::{create_deep_agent, DeepAgentOptions, backend::StateBackend};
//!
//! let backend = Arc::new(StateBackend::new());
//! let options = DeepAgentOptions::new(backend);
//! let agent = create_deep_agent(model, options)?;
//! let result = agent.invoke(MessageState::with_messages(vec![
//!     Message::human("Write hello.txt"),
//! ])).await?;
//! ```

pub mod backend;
pub mod middleware;
pub mod tools;

use std::sync::Arc;

use synaptic_core::{ChatModel, Store, SynapticError, Tool};
use synaptic_graph::{
    create_agent, AgentOptions, Checkpointer, CompiledGraph, MessageState,
};
use synaptic_macros::traceable;
use synaptic_middleware::AgentMiddleware;

use backend::Backend;
pub use middleware::subagent::SubAgentDef;

/// Configuration for [`create_deep_agent`].
pub struct DeepAgentOptions {
    /// Backend for filesystem operations.
    pub backend: Arc<dyn Backend>,
    /// Optional system prompt prepended to all model calls.
    pub system_prompt: Option<String>,
    /// Additional tools beyond the built-in filesystem tools.
    pub tools: Vec<Arc<dyn Tool>>,
    /// Additional middleware beyond the built-in stack.
    pub middleware: Vec<Arc<dyn AgentMiddleware>>,
    /// Optional checkpointer for graph state persistence.
    pub checkpointer: Option<Arc<dyn Checkpointer>>,
    /// Optional store for runtime tool injection.
    pub store: Option<Arc<dyn Store>>,
    /// Maximum input tokens before summarization (default 128,000).
    pub max_input_tokens: usize,
    /// Fraction of max_input_tokens that triggers summarization (default 0.85).
    pub summarization_threshold: f64,
    /// Token count above which tool results are evicted to files (default 20,000).
    pub eviction_threshold: usize,
    /// Maximum nested subagent depth (default 3).
    pub max_subagent_depth: usize,
    /// Skills directory path in the backend (default ".skills").
    pub skills_dir: Option<String>,
    /// Memory file path in the backend (default "AGENTS.md").
    pub memory_file: Option<String>,
    /// Custom subagent definitions for the task tool.
    pub subagents: Vec<SubAgentDef>,
    /// Enable subagent spawning via task tool (default true).
    pub enable_subagents: bool,
    /// Enable filesystem tools (default true).
    pub enable_filesystem: bool,
    /// Enable skills middleware (default true).
    pub enable_skills: bool,
    /// Enable memory middleware (default true).
    pub enable_memory: bool,
}

impl DeepAgentOptions {
    /// Create options with the given backend and sensible defaults.
    pub fn new(backend: Arc<dyn Backend>) -> Self {
        Self {
            backend,
            system_prompt: None,
            tools: Vec::new(),
            middleware: Vec::new(),
            checkpointer: None,
            store: None,
            max_input_tokens: 128_000,
            summarization_threshold: 0.85,
            eviction_threshold: 20_000,
            max_subagent_depth: 3,
            skills_dir: Some(".skills".to_string()),
            memory_file: Some("AGENTS.md".to_string()),
            subagents: Vec::new(),
            enable_subagents: true,
            enable_filesystem: true,
            enable_skills: true,
            enable_memory: true,
        }
    }
}

/// Create a deep agent with the given model and options.
///
/// Assembles a middleware stack and tool set:
/// 1. **DeepMemoryMiddleware** — loads memory file into system prompt
/// 2. **SkillsMiddleware** — progressive disclosure of skills
/// 3. **FilesystemMiddleware** — 6–7 filesystem tools + large result eviction
/// 4. **SubAgentMiddleware** — `task` tool for child agent spawning
/// 5. **DeepSummarizationMiddleware** — auto-summarize context on overflow
/// 6. **PatchToolCallsMiddleware** — fix malformed tool calls
/// 7. User-provided middleware
#[traceable(skip = "model,options")]
pub fn create_deep_agent(
    model: Arc<dyn ChatModel>,
    options: DeepAgentOptions,
) -> Result<CompiledGraph<MessageState>, SynapticError> {
    let mut all_middleware: Vec<Arc<dyn AgentMiddleware>> = Vec::new();
    let mut all_tools: Vec<Arc<dyn Tool>> = Vec::new();

    // 1. Memory middleware
    if options.enable_memory {
        let memory_file = options
            .memory_file
            .clone()
            .unwrap_or_else(|| "AGENTS.md".to_string());
        all_middleware.push(Arc::new(middleware::memory::DeepMemoryMiddleware::new(
            options.backend.clone(),
            memory_file,
        )));
    }

    // 2. Skills middleware
    if options.enable_skills {
        let skills_dir = options
            .skills_dir
            .clone()
            .unwrap_or_else(|| ".skills".to_string());
        all_middleware.push(Arc::new(middleware::skills::SkillsMiddleware::new(
            options.backend.clone(),
            skills_dir,
        )));
    }

    // 3. Filesystem middleware + tools
    if options.enable_filesystem {
        let fs_tools = tools::create_filesystem_tools(options.backend.clone());
        all_tools.extend(fs_tools);
        all_middleware.push(Arc::new(middleware::filesystem::FilesystemMiddleware::new(
            options.backend.clone(),
            options.eviction_threshold,
        )));
    }

    // 4. Subagent middleware + task tool
    if options.enable_subagents {
        let subagent_mw = middleware::subagent::SubAgentMiddleware::new(
            options.backend.clone(),
            model.clone(),
            options.max_subagent_depth,
            options.subagents.clone(),
        );
        all_tools.push(subagent_mw.create_task_tool());
    }

    // 5. Summarization middleware
    all_middleware.push(Arc::new(
        middleware::summarization::DeepSummarizationMiddleware::new(
            options.backend.clone(),
            model.clone(),
            options.max_input_tokens,
            options.summarization_threshold,
        ),
    ));

    // 6. Patch tool calls middleware
    all_middleware.push(Arc::new(
        middleware::patch_tool_calls::PatchToolCallsMiddleware,
    ));

    // 7. User-provided middleware
    all_middleware.extend(options.middleware);

    // Add user-provided tools
    all_tools.extend(options.tools);

    // Build agent options
    let agent_options = AgentOptions {
        checkpointer: options.checkpointer,
        interrupt_before: Vec::new(),
        interrupt_after: Vec::new(),
        system_prompt: options.system_prompt,
        middleware: all_middleware,
        store: options.store,
        name: Some("deep_agent".to_string()),
        pre_model_hook: None,
        post_model_hook: None,
        response_format: None,
    };

    create_agent(model, all_tools, agent_options)
}
