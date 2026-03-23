pub mod bootstrap;
mod builder;
pub mod callbacks;
pub mod context_engine;
pub mod discovery;
pub(crate) mod mcp;
pub(crate) mod middleware;
mod middleware_setup;
pub(crate) mod model;
pub mod prose_vm;
pub mod registry;
pub mod runtime;
pub mod self_awareness;
pub mod subscribers;
pub mod templates;
pub mod thinking;
#[allow(dead_code)]
pub mod tool_display;
pub mod tool_policy;
mod tools_setup;
pub mod workspace;

// Re-export public API to maintain backward-compatible import paths.
pub use self::bootstrap::{BootstrapLoader, SessionKind};
pub use self::builder::{build_deep_agent, build_deep_agent_with_callback, SessionOverrides};
pub use self::callbacks::{BotSafetyCallback, InteractiveApprovalCallback};
pub use self::mcp::{build_mcp_client, load_mcp_tools};
pub use self::model::{build_model, build_model_by_name};
