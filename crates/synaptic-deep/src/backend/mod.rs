use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use synaptic_core::SynapticError;

pub mod state;
pub mod store;

#[cfg(feature = "filesystem")]
pub mod filesystem;

pub use state::StateBackend;
pub use store::StoreBackend;

#[cfg(feature = "filesystem")]
pub use filesystem::FilesystemBackend;

/// A directory entry returned by [`Backend::ls`].
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirEntry {
    pub name: String,
    pub is_dir: bool,
    pub size: Option<u64>,
}

/// Result of a shell command execution.
#[derive(Debug, Clone)]
pub struct ExecResult {
    pub stdout: String,
    pub stderr: String,
    pub exit_code: i32,
}

/// A single grep match.
#[derive(Debug, Clone)]
pub struct GrepMatch {
    pub file: String,
    pub line_number: usize,
    pub line: String,
}

/// Output mode for grep operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrepOutputMode {
    FilesWithMatches,
    Content,
    Count,
}

/// Pluggable filesystem backend for deep agents.
///
/// Implementations provide file I/O, glob, and grep operations.
/// The default `execute` method returns an error; override it to support shell commands.
#[async_trait]
pub trait Backend: Send + Sync {
    /// List entries in a directory.
    async fn ls(&self, path: &str) -> Result<Vec<DirEntry>, SynapticError>;

    /// Read file contents with line-based pagination.
    async fn read_file(
        &self,
        path: &str,
        offset: usize,
        limit: usize,
    ) -> Result<String, SynapticError>;

    /// Create or overwrite a file.
    async fn write_file(&self, path: &str, content: &str) -> Result<(), SynapticError>;

    /// Find-and-replace text in a file.
    async fn edit_file(
        &self,
        path: &str,
        old_text: &str,
        new_text: &str,
        replace_all: bool,
    ) -> Result<(), SynapticError>;

    /// Match file paths against a glob pattern within a base directory.
    async fn glob(&self, pattern: &str, base: &str) -> Result<Vec<String>, SynapticError>;

    /// Search file contents by regex pattern.
    async fn grep(
        &self,
        pattern: &str,
        path: Option<&str>,
        file_glob: Option<&str>,
        output_mode: GrepOutputMode,
    ) -> Result<String, SynapticError>;

    /// Execute a shell command. Returns error by default.
    async fn execute(
        &self,
        _command: &str,
        _timeout: Option<Duration>,
    ) -> Result<ExecResult, SynapticError> {
        Err(SynapticError::Tool(
            "execution not supported by this backend".into(),
        ))
    }

    /// Whether this backend supports shell command execution.
    fn supports_execution(&self) -> bool {
        false
    }
}
