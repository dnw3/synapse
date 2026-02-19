# Filesystem Tools

A Deep Agent ships with six built-in filesystem tools, plus a conditional seventh. These tools are automatically registered when you call `create_deep_agent` (if `enable_filesystem` is `true`, which is the default) and are dispatched through whichever backend you configure.

## Creating the Tools

If you need the tool set outside of a `DeepAgent` (for example, in a custom graph), use the factory function:

```rust,ignore
use synaptic_deep::tools::create_filesystem_tools;
use synaptic_deep::backend::FilesystemBackend;
use std::sync::Arc;

let backend = Arc::new(FilesystemBackend::new("/workspace"));
let tools = create_filesystem_tools(backend);
// tools: Vec<Arc<dyn Tool>>
// 6 tools always: ls, read_file, write_file, edit_file, glob, grep
// + execute (only if backend.supports_execution() returns true)
```

The `execute` tool is only included when the backend reports that it supports execution. For `FilesystemBackend` this is always the case. For `StateBackend` and `StoreBackend`, execution is not supported and the tool is omitted.

## Tool Reference

| Tool | Description | Always present |
|------|-------------|----------------|
| `ls` | List directory contents | Yes |
| `read_file` | Read file contents with optional line-based pagination | Yes |
| `write_file` | Create or overwrite a file | Yes |
| `edit_file` | Find and replace text in an existing file | Yes |
| `glob` | Find files matching a glob pattern | Yes |
| `grep` | Search file contents by regex pattern | Yes |
| `execute` | Run a shell command and capture output | Only if backend supports execution |

### ls

Lists files and directories at the given path.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | string | yes | Directory to list |

Returns a JSON array of entries, each with `name` (string), `is_dir` (boolean), and `size` (integer or null) fields.

### read_file

Reads the contents of a single file with line-based pagination.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | string | yes | File path to read |
| `offset` | integer | no | Starting line number, 0-based (default 0) |
| `limit` | integer | no | Maximum number of lines to return (default 2000) |

Returns the file contents as a string. When `offset` and `limit` are provided, returns only the requested line range.

### write_file

Creates a new file or overwrites an existing one.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | string | yes | Destination file path |
| `content` | string | yes | Full file contents to write |

Returns a confirmation string (e.g. `"wrote path/to/file"`).

### edit_file

Applies a targeted string replacement within an existing file.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | string | yes | File to edit |
| `old_string` | string | yes | Exact text to find |
| `new_string` | string | yes | Replacement text |
| `replace_all` | boolean | no | Replace all occurrences (default false) |

When `replace_all` is `false` (the default), only the first occurrence is replaced. The tool returns an error if `old_string` is not found in the file.

### glob

Finds files matching a glob pattern.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `pattern` | string | yes | Glob pattern (e.g. `"**/*.rs"`, `"src/*.toml"`) |
| `path` | string | no | Base directory to search from (default `"."`) |

Returns matching file paths as a newline-separated string.

### grep

Searches file contents for lines matching a regular expression.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `pattern` | string | yes | Regex pattern to search for |
| `path` | string | no | Directory or file to search in (defaults to workspace root) |
| `glob` | string | no | Glob pattern to filter which files are searched (e.g. `"*.rs"`) |
| `output_mode` | string | no | Output format: `"files_with_matches"` (default), `"content"`, or `"count"` |

Output modes control the format of results:

- **`files_with_matches`** -- Returns one matching file path per line.
- **`content`** -- Returns matching lines in `file:line_number:line` format.
- **`count`** -- Returns match counts in `file:count` format.

### execute

Runs a shell command in the backend's working directory. This tool is only registered when the backend supports execution (i.e. `FilesystemBackend`).

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `command` | string | yes | The shell command to execute |
| `timeout` | integer | no | Timeout in seconds |

Returns a JSON object with `stdout`, `stderr`, and `exit_code` fields. Commands are executed via `sh -c` in the backend's root directory.
