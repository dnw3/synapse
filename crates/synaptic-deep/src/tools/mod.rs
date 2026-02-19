use serde_json::{json, Value};
use std::sync::Arc;
use synaptic_core::{SynapticError, Tool};
use synaptic_macros::{tool, traceable};

use crate::backend::{Backend, GrepOutputMode};

/// Create the built-in filesystem tools backed by the given backend.
///
/// Returns 6 tools (ls, read_file, write_file, edit_file, glob, grep) plus
/// an `execute` tool if the backend supports execution.
#[traceable(skip = "backend")]
pub fn create_filesystem_tools(backend: Arc<dyn Backend>) -> Vec<Arc<dyn Tool>> {
    let mut tools: Vec<Arc<dyn Tool>> = vec![
        ls(backend.clone()),
        read_file(backend.clone()),
        write_file(backend.clone()),
        edit_file(backend.clone()),
        glob_files(backend.clone()),
        grep(backend.clone()),
    ];
    if backend.supports_execution() {
        tools.push(execute(backend));
    }
    tools
}

/// List directory contents
#[tool]
async fn ls(
    #[field] backend: Arc<dyn Backend>,
    /// Directory path to list
    path: String,
) -> Result<Value, SynapticError> {
    let entries = backend.ls(&path).await?;
    serde_json::to_value(entries)
        .map_err(|e| SynapticError::Tool(format!("serialization: {}", e)))
}

/// Read file contents with optional line-based pagination
#[tool]
async fn read_file(
    #[field] backend: Arc<dyn Backend>,
    /// File path to read
    path: String,
    /// Starting line number (0-based, default 0)
    #[default = 0]
    offset: usize,
    /// Maximum lines to read (default 2000)
    #[default = 2000]
    limit: usize,
) -> Result<Value, SynapticError> {
    let content = backend.read_file(&path, offset, limit).await?;
    Ok(Value::String(content))
}

/// Create or overwrite a file with the given content
#[tool]
async fn write_file(
    #[field] backend: Arc<dyn Backend>,
    /// File path to write
    path: String,
    /// Content to write
    content: String,
) -> Result<Value, SynapticError> {
    backend.write_file(&path, &content).await?;
    Ok(Value::String(format!("wrote {}", path)))
}

/// Find and replace text in a file
#[tool]
async fn edit_file(
    #[field] backend: Arc<dyn Backend>,
    /// File path to edit
    path: String,
    /// Text to find
    old_string: String,
    /// Replacement text
    new_string: String,
    /// Replace all occurrences (default false)
    #[default = false]
    replace_all: bool,
) -> Result<Value, SynapticError> {
    backend
        .edit_file(&path, &old_string, &new_string, replace_all)
        .await?;
    Ok(Value::String(format!("edited {}", path)))
}

/// Find files matching a glob pattern
#[tool(name = "glob")]
async fn glob_files(
    #[field] backend: Arc<dyn Backend>,
    /// Glob pattern (e.g. **/*.rs)
    pattern: String,
    /// Base directory (default .)
    #[default = ".".to_string()]
    path: String,
) -> Result<Value, SynapticError> {
    let matches = backend.glob(&pattern, &path).await?;
    Ok(Value::String(matches.join("\n")))
}

/// Search file contents by regex pattern
#[tool]
async fn grep(
    #[field] backend: Arc<dyn Backend>,
    /// Regex pattern to search for
    pattern: String,
    /// Directory or file to search in
    path: Option<String>,
    /// Glob pattern to filter files
    glob: Option<String>,
    /// Output format: files_with_matches (default), content, count
    output_mode: Option<String>,
) -> Result<Value, SynapticError> {
    let mode = match output_mode.as_deref() {
        Some("content") => GrepOutputMode::Content,
        Some("count") => GrepOutputMode::Count,
        _ => GrepOutputMode::FilesWithMatches,
    };
    let result = backend
        .grep(&pattern, path.as_deref(), glob.as_deref(), mode)
        .await?;
    Ok(Value::String(result))
}

/// Execute a shell command
#[tool]
async fn execute(
    #[field] backend: Arc<dyn Backend>,
    /// Shell command to execute
    command: String,
    /// Timeout in seconds
    timeout: Option<u64>,
) -> Result<Value, SynapticError> {
    let duration = timeout.map(std::time::Duration::from_secs);
    let result = backend.execute(&command, duration).await?;
    Ok(json!({
        "stdout": result.stdout,
        "stderr": result.stderr,
        "exit_code": result.exit_code,
    }))
}
