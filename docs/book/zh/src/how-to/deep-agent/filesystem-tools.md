# 文件系统工具

Deep Agent 内置了六个文件系统工具，外加一个条件性的第七个。当你调用 `create_deep_agent` 时（如果 `enable_filesystem` 为 `true`，这是默认值），这些工具会自动注册，并通过你配置的 `backend` 进行调度。

## 创建工具

如果你需要在 `DeepAgent` 之外使用这些工具（例如在自定义图中），可以使用工厂函数：

```rust,ignore
use synaptic_deep::tools::create_filesystem_tools;
use synaptic_deep::backend::FilesystemBackend;
use std::sync::Arc;

let backend = Arc::new(FilesystemBackend::new("/workspace"));
let tools = create_filesystem_tools(backend);
// tools: Vec<Arc<dyn Tool>>
// 始终包含 6 个工具：ls, read_file, write_file, edit_file, glob, grep
// + execute（仅当 backend.supports_execution() 返回 true 时）
```

`execute` 工具仅在 `backend` 报告支持执行时才包含。对于 `FilesystemBackend`，始终如此。对于 `StateBackend` 和 `StoreBackend`，不支持执行，因此该工具会被省略。

## 工具参考

| 工具 | 描述 | 始终存在 |
|------|------|----------|
| `ls` | 列出目录内容 | 是 |
| `read_file` | 读取文件内容，支持可选的基于行的分页 | 是 |
| `write_file` | 创建或覆盖文件 | 是 |
| `edit_file` | 在现有文件中查找并替换文本 | 是 |
| `glob` | 查找匹配 glob 模式的文件 | 是 |
| `grep` | 按正则表达式搜索文件内容 | 是 |
| `execute` | 运行 shell 命令并捕获输出 | 仅当 `backend` 支持执行时 |

### ls

列出给定路径下的文件和目录。

| 参数 | 类型 | 必填 | 描述 |
|------|------|------|------|
| `path` | string | 是 | 要列出的目录 |

返回一个 JSON 数组，每个条目包含 `name`（字符串）、`is_dir`（布尔值）和 `size`（整数或 null）字段。

### read_file

读取单个文件的内容，支持基于行的分页。

| 参数 | 类型 | 必填 | 描述 |
|------|------|------|------|
| `path` | string | 是 | 要读取的文件路径 |
| `offset` | integer | 否 | 起始行号，从 0 开始（默认 0） |
| `limit` | integer | 否 | 返回的最大行数（默认 2000） |

返回文件内容字符串。当提供 `offset` 和 `limit` 时，仅返回请求的行范围。

### write_file

创建新文件或覆盖已有文件。

| 参数 | 类型 | 必填 | 描述 |
|------|------|------|------|
| `path` | string | 是 | 目标文件路径 |
| `content` | string | 是 | 要写入的完整文件内容 |

返回确认字符串（例如 `"wrote path/to/file"`）。

### edit_file

在现有文件中执行定向字符串替换。

| 参数 | 类型 | 必填 | 描述 |
|------|------|------|------|
| `path` | string | 是 | 要编辑的文件 |
| `old_string` | string | 是 | 要查找的精确文本 |
| `new_string` | string | 是 | 替换文本 |
| `replace_all` | boolean | 否 | 是否替换所有出现（默认 false） |

当 `replace_all` 为 `false`（默认值）时，只替换第一次出现。如果文件中未找到 `old_string`，工具将返回错误。

### glob

查找匹配 glob 模式的文件。

| 参数 | 类型 | 必填 | 描述 |
|------|------|------|------|
| `pattern` | string | 是 | Glob 模式（例如 `"**/*.rs"`、`"src/*.toml"`） |
| `path` | string | 否 | 搜索的基准目录（默认 `"."`） |

返回匹配的文件路径，以换行符分隔的字符串。

### grep

搜索文件内容中匹配正则表达式的行。

| 参数 | 类型 | 必填 | 描述 |
|------|------|------|------|
| `pattern` | string | 是 | 要搜索的正则表达式 |
| `path` | string | 否 | 要搜索的目录或文件（默认为工作空间根目录） |
| `glob` | string | 否 | 用于过滤搜索文件的 glob 模式（例如 `"*.rs"`） |
| `output_mode` | string | 否 | 输出格式：`"files_with_matches"`（默认）、`"content"` 或 `"count"` |

输出模式控制结果的格式：

- **`files_with_matches`** -- 每行返回一个匹配的文件路径。
- **`content`** -- 以 `file:line_number:line` 格式返回匹配的行。
- **`count`** -- 以 `file:count` 格式返回匹配计数。

### execute

在 `backend` 的工作目录中运行 shell 命令。此工具仅在 `backend` 支持执行时注册（即 `FilesystemBackend`）。

| 参数 | 类型 | 必填 | 描述 |
|------|------|------|------|
| `command` | string | 是 | 要执行的 shell 命令 |
| `timeout` | integer | 否 | 超时时间（秒） |

返回一个包含 `stdout`、`stderr` 和 `exit_code` 字段的 JSON 对象。命令通过 `sh -c` 在 `backend` 的根目录中执行。
