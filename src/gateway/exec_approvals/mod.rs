//! Exec approvals subsystem for command execution gating.

pub mod config;
pub mod manager;
pub mod policy;

pub use config::ExecApprovalsConfig;
pub use manager::ExecApprovalManager;
