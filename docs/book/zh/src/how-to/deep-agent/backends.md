# Backend

Deep Agent 的 `backend` 控制文件系统工具如何与外部世界交互。Synaptic 提供了三种内置 `backend`，你可以根据部署场景选择合适的一种。

## StateBackend

完全基于内存的 `backend`。文件存储在以标准化路径为键的 `HashMap<String, String>` 中，不会接触真实文件系统。目录通过路径前缀推断，而非作为显式条目存储。这是测试和沙箱演示的默认选择。

```rust,ignore
use synaptic_deep::backend::StateBackend;
use std::sync::Arc;

let backend = Arc::new(StateBackend::new());

let options = DeepAgentOptions::new(backend.clone());
let agent = create_deep_agent(model, options)?;

// agent 运行后，检查虚拟文件系统：
let entries = backend.ls("/").await?;
let content = backend.read_file("/hello.txt", 0, 2000).await?;
```

`StateBackend` 不支持 shell 命令执行——`supports_execution()` 返回 `false`，`execute()` 返回错误。

**适用场景：** 单元测试、CI 流水线、不应发生真实 I/O 的沙箱环境。

## StoreBackend

通过 Synaptic 的 `Store` trait 持久化文件。每个文件以 `key=path`、`value={"content": "..."}` 的形式存储为一个条目。所有条目共享一个可配置的命名空间前缀。这使你可以用任何存储实现来支撑 agent 的工作空间——开发时使用 `InMemoryStore`，生产环境使用自定义的数据库存储。

```rust,ignore
use synaptic_deep::backend::StoreBackend;
use synaptic_store::InMemoryStore;
use std::sync::Arc;

let store = Arc::new(InMemoryStore::new());
let namespace = vec!["workspace".to_string(), "agent1".to_string()];
let backend = Arc::new(StoreBackend::new(store, namespace));

let options = DeepAgentOptions::new(backend);
let agent = create_deep_agent(model, options)?;
```

第二个参数是一个 `Vec<String>` 命名空间。所有文件键都存储在该命名空间下，因此多个 agent 可以共享同一个 store 而不会发生键冲突。

`StoreBackend` 不支持 shell 命令执行——`supports_execution()` 返回 `false`，`execute()` 返回错误。

**适用场景：** 希望实现持久化但不授予直接文件系统访问权限的服务端部署。非常适合多租户应用。

## FilesystemBackend

读写宿主操作系统上的真实文件。这是编程助手和本地自动化场景所需的 `backend`。

```rust,ignore
use synaptic_deep::backend::FilesystemBackend;
use std::sync::Arc;

let backend = Arc::new(FilesystemBackend::new("/home/user/project"));

let options = DeepAgentOptions::new(backend);
let agent = create_deep_agent(model, options)?;
```

你提供的路径将成为 agent 的根目录。所有工具路径都相对于此根目录解析。Agent 无法逃出根目录——包含 `..` 的路径会被拒绝。

`FilesystemBackend` 是唯一支持 shell 命令执行的内置 `backend`。命令通过 `sh -c` 在根目录中运行，并支持可选的超时设置。使用此 `backend` 时，`create_filesystem_tools` 会自动包含 `execute` 工具。

> **Feature 门控：** `FilesystemBackend` 需要启用 `filesystem` Cargo feature：
>
> ```toml
> synaptic-deep = { path = "../crates/synaptic-deep", features = ["filesystem"] }
> ```

**适用场景：** 本地 CLI 工具、编程助手，以及任何 agent 需要与真实文件交互的场景。

## 实现自定义 Backend

三种 `backend` 都实现了 `synaptic_deep::backend` 中的 `Backend` trait：

```rust,ignore
use synaptic_deep::backend::{Backend, DirEntry, ExecResult, GrepOutputMode};

#[async_trait]
pub trait Backend: Send + Sync {
    /// 列出目录中的条目。
    async fn ls(&self, path: &str) -> Result<Vec<DirEntry>, SynapticError>;

    /// 读取文件内容，支持基于行的分页。
    async fn read_file(&self, path: &str, offset: usize, limit: usize)
        -> Result<String, SynapticError>;

    /// 创建或覆盖文件。
    async fn write_file(&self, path: &str, content: &str) -> Result<(), SynapticError>;

    /// 在文件中查找并替换文本。
    async fn edit_file(&self, path: &str, old_text: &str, new_text: &str, replace_all: bool)
        -> Result<(), SynapticError>;

    /// 在基准目录内按 glob 模式匹配文件路径。
    async fn glob(&self, pattern: &str, base: &str) -> Result<Vec<String>, SynapticError>;

    /// 按正则表达式搜索文件内容。
    async fn grep(&self, pattern: &str, path: Option<&str>, file_glob: Option<&str>,
        output_mode: GrepOutputMode) -> Result<String, SynapticError>;

    /// 执行 shell 命令。默认返回错误。
    async fn execute(&self, command: &str, timeout: Option<Duration>)
        -> Result<ExecResult, SynapticError> { /* 默认：错误 */ }

    /// 此 backend 是否支持 shell 命令执行。
    fn supports_execution(&self) -> bool { false }
}
```

相关类型：

- `DirEntry` -- `{ name: String, is_dir: bool, size: Option<u64> }`
- `ExecResult` -- `{ stdout: String, stderr: String, exit_code: i32 }`
- `GrepMatch` -- `{ file: String, line_number: usize, line: String }`
- `GrepOutputMode` -- `FilesWithMatches | Content | Count`

实现此 trait 可以将 agent 的存储后端替换为 S3、数据库、通过 SSH 连接的远程服务器，或任何其他存储层。如果你希望为自定义 `backend` 启用 `execute` 工具，请重写 `execute` 和 `supports_execution` 方法。

## 离线测试

使用 `StateBackend` 配合 `ScriptedChatModel` 来测试 Deep Agent，无需 API 密钥或真实文件系统访问：

```rust,ignore
use std::sync::Arc;
use synaptic_core::{ChatResponse, Message, ToolCall};
use synaptic_models::ScriptedChatModel;
use synaptic_deep::{create_deep_agent, DeepAgentOptions};
use synaptic_deep::backend::StateBackend;

// 脚本化模型：先写入文件，然后结束
let model = Arc::new(ScriptedChatModel::new(vec![
    ChatResponse {
        message: Message::ai_with_tool_calls(
            "I'll create a file.",
            vec![ToolCall {
                id: "call_1".into(),
                name: "write_file".into(),
                arguments: r#"{"path": "/hello.txt", "content": "Hello from test!"}"#.into(),
            }],
        ),
        usage: None,
    },
    ChatResponse {
        message: Message::ai("Done! I created hello.txt."),
        usage: None,
    },
]));

let backend = Arc::new(StateBackend::new());
let options = DeepAgentOptions::new(backend.clone());
let agent = create_deep_agent(model, options)?;

// 运行 agent...
// 然后检查虚拟文件系统：
let content = backend.read_file("/hello.txt", 0, 2000).await?;
assert!(content.contains("Hello from test!"));
```

这种模式非常适合 CI 流水线和单元测试。`StateBackend` 完全确定性且无需清理。

## 对比

| Backend | 持久化 | 真实 I/O | 命令执行 | Feature 门控 | 最适用于 |
|---------|--------|----------|----------|-------------|----------|
| `StateBackend` | 无（内存） | 否 | 否 | 无 | 测试、沙箱 |
| `StoreBackend` | 通过 `Store` trait | 否 | 否 | 无 | 服务端、多租户 |
| `FilesystemBackend` | 磁盘 | 是 | 是 | `filesystem` | 本地 CLI、编程助手 |
