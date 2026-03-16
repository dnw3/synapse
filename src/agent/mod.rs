mod builder;
pub mod callbacks;
mod context;
pub mod copilot_proxy;
pub mod discovery;
pub mod link_understanding;
mod mcp;
pub(crate) mod middleware;
mod model;
pub mod registry;
pub mod runtime;
pub mod self_awareness;
pub mod subscribers;
pub mod templates;
pub mod thinking;
pub mod tool_policy;
pub mod tracing_mw;
pub mod workspace;

// Re-export public API to maintain backward-compatible import paths.
pub use self::builder::{build_deep_agent, build_deep_agent_with_callback, SessionOverrides};
pub use self::callbacks::{BotSafetyCallback, InteractiveApprovalCallback};
pub use self::context::load_project_context;
pub use self::mcp::load_mcp_tools;
pub use self::model::{build_model, build_model_by_name};
