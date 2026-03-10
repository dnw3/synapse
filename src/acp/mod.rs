//! Agent Communication Protocol (ACP) transport layer.
//!
//! Supports stdio (LSP-style) and HTTP transports for JSON-RPC 2.0 based
//! agent communication.

#[cfg(feature = "web")]
pub mod server;
pub mod stdio;
