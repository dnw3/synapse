# 构建 Deep Agent

本教程将带你一步步构建一个 Deep Agent。你将从一个能读写文件的最小 Agent 开始，逐步添加技能、子 Agent、记忆和自定义配置。学完本教程后，你将理解 Deep Agent 技术栈的每一层。

## 你将构建什么

一个 Deep Agent，它能够：

1. 使用文件系统工具读取、写入和搜索文件。
2. 从 `SKILL.md` 文件加载领域特定的技能。
3. 将子任务委派给自定义子 Agent。
4. 将学到的知识持久化到 `AGENTS.md` 记忆文件中。
5. 当上下文增长过大时自动摘要对话历史。

## 前置条件

创建一个新的二进制 crate：

```bash
cargo new deep-agent-tutorial
cd deep-agent-tutorial
```

在 `Cargo.toml` 中添加依赖：

```toml
[dependencies]
synaptic-deep = { path = "../crates/synaptic-deep" }
synaptic-core = { path = "../crates/synaptic-core" }
synaptic-graph = { path = "../crates/synaptic-graph" }
synaptic-models = { path = "../crates/synaptic-models" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

设置你的 OpenAI API 密钥：

```bash
export OPENAI_API_KEY="sk-..."
```

## 第 1 步：创建 Backend

每个 Deep Agent 都需要一个 **Backend**，用于提供文件系统操作。Backend 是 Agent 对外部世界的视图——它决定了文件从哪里读取、写入到哪里。

Synaptic 提供三种 Backend 实现：

- **`StateBackend`** -- 基于内存的 `HashMap<String, String>`。非常适合测试和沙箱演示。不会触及真实文件。
- **`StoreBackend`** -- 委派给 Synaptic 的 `Store` 实现。当你已经有一个支持语义搜索的 Store 时很有用。
- **`FilesystemBackend`** -- 在磁盘上读写真实文件，沙箱化到一个根目录。需要启用 `filesystem` feature flag。

本教程使用 `StateBackend`，所有操作都在内存中运行：

```rust,ignore
use std::sync::Arc;
use synaptic_deep::backend::{Backend, StateBackend};

let backend = Arc::new(StateBackend::new());
```

Deep Agent 将每个 Backend 操作封装为模型可以调用的工具。

## 第 2 步：创建最小的 Deep Agent

`create_deep_agent` 函数在一次调用中组装完整的中间件栈和工具集。它返回一个 `CompiledGraph<MessageState>` —— 与 `create_agent` 和 `create_react_agent` 使用的相同图类型，因此你可以用 `invoke()` 来运行它。

```rust,ignore
use std::sync::Arc;
use synaptic_deep::{create_deep_agent, DeepAgentOptions};
use synaptic_deep::backend::StateBackend;
use synaptic_core::{ChatModel, Message};
use synaptic_graph::MessageState;
use synaptic_models::OpenAiChatModel;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let model: Arc<dyn ChatModel> = Arc::new(OpenAiChatModel::new("gpt-4o"));
    let backend = Arc::new(StateBackend::new());

    let options = DeepAgentOptions::new(backend.clone());
    let agent = create_deep_agent(model.clone(), options)?;

    let state = MessageState::with_messages(vec![
        Message::human("Create a file called hello.txt with 'Hello World!'"),
    ]);
    let result = agent.invoke(state).await?;
    let final_state = result.into_state();
    println!("{}", final_state.last_message().unwrap().content());

    Ok(())
}
```

底层发生了什么：

1. `DeepAgentOptions::new(backend)` 配置了合理的默认值——启用文件系统工具、技能、记忆和子 Agent。
2. `create_deep_agent` 组装 6 个中间件层和 6-7 个工具，然后调用 `create_agent` 生成编译后的图。
3. `agent.invoke(state)` 运行 Agent 循环。模型看到 `write_file` 工具并调用它在 Backend 中创建 `hello.txt`。
4. `result.into_state()` 将 `GraphResult` 解包为最终的 `MessageState`。

因为我们使用的是 `StateBackend`，文件仅存在于内存中。你可以验证它：

```rust,ignore
let content = backend.read_file("hello.txt", 0, 100).await?;
assert!(content.contains("Hello World!"));
```

## 第 3 步：使用文件系统工具

Deep Agent 自动注册以下工具：`ls`、`read_file`、`write_file`、`edit_file`、`glob`、`grep`，以及 `execute`（如果 Backend 支持 shell 命令）。

让我们向 Backend 中写入一个小型 Rust 项目，然后让 Agent 分析它：

```rust,ignore
// Seed files into the in-memory backend
backend.write_file("src/main.rs", r#"fn main() {
    let items = vec![1, 2, 3, 4, 5];
    let mut total = 0;
    for i in items {
        total = total + i;
    }
    println!("Total: {}", total);
    // TODO: add error handling
    // TODO: extract into a function
}
"#).await?;

backend.write_file("Cargo.toml", r#"[package]
name = "sample"
version = "0.1.0"
edition = "2021"
"#).await?;

let state = MessageState::with_messages(vec![
    Message::human("Read src/main.rs. List all the TODO comments and suggest improvements."),
]);
let result = agent.invoke(state).await?;
let final_state = result.into_state();
println!("{}", final_state.last_message().unwrap().content());
```

Agent 调用 `read_file` 获取源代码，找到 TODO 注释，并给出改进建议。你可以接着发出写入请求：

```rust,ignore
let state = MessageState::with_messages(vec![
    Message::human(
        "Create src/lib.rs with a public function `sum_items(items: &[i32]) -> i32` \
         that uses iter().sum(). Then update src/main.rs to use it."
    ),
]);
let result = agent.invoke(state).await?;
```

Agent 使用 `write_file` 和 `edit_file` 完成修改。

## 第 4 步：添加技能

技能是存储为 `SKILL.md` 文件的领域特定指令，保存在 Backend 中。`SkillsMiddleware` 在每次模型调用时扫描 `{skills_dir}/*/SKILL.md`，解析 YAML frontmatter 中的 `name` 和 `description`，并将技能索引注入系统提示。然后 Agent 可以通过 `read_file` 读取任何技能的完整详情。

直接向 Backend 写入一个技能文件：

```rust,ignore
backend.write_file(
    ".skills/testing/SKILL.md",
    "---\nname: testing\ndescription: Write comprehensive tests\n---\n\
     # Testing Skill\n\n\
     When asked to test Rust code:\n\n\
     1. Create a `tests/` module with `#[cfg(test)]`.\n\
     2. Write at least one happy-path test and one edge-case test.\n\
     3. Use `assert_eq!` with descriptive messages.\n\
     4. Test error paths with `assert!(result.is_err())`.\n"
).await?;
```

技能默认启用（`enable_skills = true`）。当 Agent 处理请求时，它会在系统提示中看到技能索引：

```text
<available_skills>
- **testing**: Write comprehensive tests (read `.skills/testing/SKILL.md` for details)
</available_skills>
```

Agent 可以调用 `read_file` 读取 `.skills/testing/SKILL.md` 获取完整指令。这是渐进式披露——索引始终很小，完整的技能内容按需加载。

你可以添加多个技能：

```rust,ignore
backend.write_file(
    ".skills/refactoring/SKILL.md",
    "---\nname: refactoring\ndescription: Rust refactoring best practices\n---\n\
     # Refactoring Skill\n\n\
     1. Prefer `iter().sum()` over manual loops.\n\
     2. Add `#[must_use]` to pure functions.\n\
     3. Run clippy before and after changes.\n"
).await?;
```

## 第 5 步：添加自定义子 Agent

Deep Agent 可以通过 `task` 工具生成子 Agent。每个子 Agent 拥有自己的对话、运行相同的中间件栈，并向父 Agent 返回摘要。

使用 `SubAgentDef` 定义自定义子 Agent 类型：

```rust,ignore
use synaptic_deep::SubAgentDef;

let mut options = DeepAgentOptions::new(backend.clone());
options.subagents = vec![SubAgentDef {
    name: "researcher".to_string(),
    description: "Research specialist".to_string(),
    system_prompt: "You are a research assistant. Use grep and read_file to \
                    find information in the codebase. Report findings concisely."
        .to_string(),
    tools: vec![], // inherits filesystem tools from the deep agent
}];
let agent = create_deep_agent(model.clone(), options)?;
```

当模型调用 `task` 工具时，它传入 `description` 和可选的 `agent_type`。如果 `agent_type` 匹配某个 `SubAgentDef` 的名称，子 Agent 就使用该定义的系统提示和额外工具。否则会生成一个通用子 Agent。

子 Agent 深度受 `max_subagent_depth`（默认 3）限制，以防止无限递归。你可以完全禁用子 Agent：

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());
options.enable_subagents = false;
let agent = create_deep_agent(model.clone(), options)?;
```

## 第 6 步：添加记忆持久化

`DeepMemoryMiddleware` 在每次模型调用时从 Backend 加载记忆文件，并将其注入系统提示中，包裹在 `<agent_memory>` 标签内。写入一个初始记忆文件：

```rust,ignore
backend.write_file(
    "AGENTS.md",
    "# Agent Memory\n\n\
     - Always use Rust idioms\n\
     - Prefer async/await over blocking I/O\n\
     - User prefers 4-space indentation\n"
).await?;

let mut options = DeepAgentOptions::new(backend.clone());
options.enable_memory = true; // this is already the default
let agent = create_deep_agent(model.clone(), options)?;
```

Agent 现在在每次调用时都会在系统提示中看到：

```text
<agent_memory>
# Agent Memory

- Always use Rust idioms
- Prefer async/await over blocking I/O
- User prefers 4-space indentation
</agent_memory>
```

记忆文件路径默认为 `"AGENTS.md"`。你可以更改它：

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());
options.memory_file = Some("project-notes.md".to_string());
```

Agent 可以通过调用 `write_file` 或 `edit_file` 更新记忆文件。未来的会话将自动获取这些更改。

## 第 7 步：自定义选项

`DeepAgentOptions` 让你可以控制整个 Agent 栈：

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());

// System prompt prepended to all model calls
options.system_prompt = Some("You are a coding assistant.".to_string());

// Token budget and summarization
options.max_input_tokens = 128_000;       // default
options.summarization_threshold = 0.85;   // default (85% of max)
options.eviction_threshold = 20_000;      // evict large tool results (default)

// Subagent configuration
options.max_subagent_depth = 3;           // default
options.enable_subagents = true;          // default

// Feature toggles
options.enable_filesystem = true;         // default
options.enable_skills = true;             // default
options.enable_memory = true;             // default

// Paths in the backend
options.skills_dir = Some(".skills".to_string());    // default
options.memory_file = Some("AGENTS.md".to_string()); // default

// Extensibility: add your own tools, middleware, checkpointer, or store
options.tools = vec![];
options.middleware = vec![];
options.checkpointer = None;
options.store = None;
options.subagents = vec![];

let agent = create_deep_agent(model.clone(), options)?;
```

## 第 8 步：完整示例

以下是一个结合所有内容的完整示例：

```rust,ignore
use std::sync::Arc;
use synaptic_deep::{create_deep_agent, DeepAgentOptions, SubAgentDef};
use synaptic_deep::backend::StateBackend;
use synaptic_core::{ChatModel, Message};
use synaptic_graph::MessageState;
use synaptic_models::OpenAiChatModel;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let model: Arc<dyn ChatModel> = Arc::new(OpenAiChatModel::new("gpt-4o"));
    let backend = Arc::new(StateBackend::new());

    // Seed the workspace
    backend.write_file("src/main.rs", "fn main() {\n    println!(\"hello\");\n}\n").await?;

    // Add a skill
    backend.write_file(
        ".skills/testing/SKILL.md",
        "---\nname: testing\ndescription: Write comprehensive tests\n---\n# Testing\nAlways write unit tests.\n"
    ).await?;

    // Add agent memory
    backend.write_file("AGENTS.md", "# Memory\n- Use Rust 2021 edition\n").await?;

    // Configure the deep agent
    let mut options = DeepAgentOptions::new(backend.clone());
    options.system_prompt = Some("You are a senior Rust engineer. Be concise.".to_string());
    options.max_input_tokens = 64_000;
    options.summarization_threshold = 0.80;
    options.max_subagent_depth = 2;
    options.subagents = vec![SubAgentDef {
        name: "researcher".to_string(),
        description: "Code research specialist".to_string(),
        system_prompt: "You research codebases and report findings.".to_string(),
        tools: vec![],
    }];

    let agent = create_deep_agent(model, options)?;

    // Run the agent
    let state = MessageState::with_messages(vec![
        Message::human(
            "Audit this project: read all source files, find TODOs, \
             and write a summary to REPORT.md."
        ),
    ]);
    let result = agent.invoke(state).await?;
    let final_state = result.into_state();
    println!("{}", final_state.last_message().unwrap().content());

    // Verify the report was created
    let report = backend.read_file("REPORT.md", 0, 100).await?;
    println!("--- REPORT.md ---\n{}", report);

    Ok(())
}
```

## 中间件栈的工作原理

`create_deep_agent` 按以下顺序组装中间件栈：

1. **DeepMemoryMiddleware** -- 读取 `AGENTS.md` 并将其追加到系统提示中。
2. **SkillsMiddleware** -- 扫描 `.skills/*/SKILL.md` 并将技能索引注入系统提示。
3. **FilesystemMiddleware** -- 注册文件系统工具。将大于 `eviction_threshold` token 的结果驱逐到 `.evicted/` 文件中，并提供预览。
4. **SubAgentMiddleware** -- 提供 `task` 工具用于生成子 Agent。
5. **DeepSummarizationMiddleware** -- 当 token 数超过阈值时摘要较旧的消息，将完整历史保存到 `.context/history_N.md`。
6. **PatchToolCallsMiddleware** -- 修复格式错误的工具调用（去除代码围栏、去重 ID、移除空名称）。
7. **用户中间件** -- `options.middleware` 中的任何内容最后运行。

## 使用真实文件系统 Backend

在生产环境中，启用 `filesystem` feature 以使用真实文件：

```toml
[dependencies]
synaptic-deep = { path = "../crates/synaptic-deep", features = ["filesystem"] }
```

```rust,ignore
use synaptic_deep::backend::FilesystemBackend;

let backend = Arc::new(FilesystemBackend::new("/path/to/workspace"));
let options = DeepAgentOptions::new(backend.clone());
let agent = create_deep_agent(model, options)?;
```

`FilesystemBackend` 将所有操作沙箱化到根目录。通过 `..` 的路径遍历会被拒绝。它还支持通过 `execute` 工具执行 shell 命令。

## 离线模式（无需 API 密钥）

对于测试和 CI，将 `StateBackend` 与 `ScriptedChatModel` 结合使用，可以在无网络访问的情况下运行整个 Deep Agent：

```rust,ignore
use std::sync::Arc;
use synaptic_core::{ChatModel, ChatResponse, Message, ToolCall};
use synaptic_models::ScriptedChatModel;
use synaptic_deep::{create_deep_agent, DeepAgentOptions};
use synaptic_deep::backend::StateBackend;
use synaptic_graph::MessageState;

let backend = Arc::new(StateBackend::new());

// Script the model to: 1) write a file, 2) respond
let model: Arc<dyn ChatModel> = Arc::new(ScriptedChatModel::new(vec![
    ChatResponse {
        message: Message::ai_with_tool_calls(
            "Creating the file.",
            vec![ToolCall {
                id: "call_1".into(),
                name: "write_file".into(),
                arguments: r#"{"path": "/output.txt", "content": "Hello from offline test!"}"#.into(),
            }],
        ),
        usage: None,
    },
    ChatResponse {
        message: Message::ai("Done! Created output.txt."),
        usage: None,
    },
]));

let options = DeepAgentOptions::new(backend.clone());
let agent = create_deep_agent(model, options)?;

let state = MessageState::with_messages(vec![
    Message::human("Create output.txt with a greeting."),
]);
let result = agent.invoke(state).await?.into_state();

// Verify the file was created in the virtual filesystem
let content = backend.read_file("/output.txt", 0, 100).await?;
assert!(content.contains("Hello from offline test!"));
```

这种方式非常适合：
- **单元测试** -- 确定性、无 API 费用、执行快速
- **CI 流水线** -- 无需密钥
- **演示** -- 无需配置即可在任何地方运行

## 你构建了什么

在本教程的过程中，你：

1. 创建了一个 `StateBackend` 作为 Agent 的内存文件系统。
2. 使用 `create_deep_agent` 组装了一个包含工具和中间件的完整 Agent。
3. 通过 `invoke()` 在 `MessageState` 上运行 Agent，并用 `into_state()` 提取结果。
4. 注册了内置文件系统工具（`ls`、`read_file`、`write_file`、`edit_file`、`glob`、`grep`）。
5. 通过带 YAML frontmatter 的 `SKILL.md` 文件添加了领域技能。
6. 使用 `SubAgentDef` 定义了自定义子 Agent 用于任务委派。
7. 通过 `AGENTS.md` 启用了持久化记忆。
8. 通过 `DeepAgentOptions` 自定义了所有选项。

## 下一步

- [多 Agent 模式](../how-to/multi-agent/index.md) -- supervisor 和 swarm 架构
- [中间件](../how-to/middleware/index.md) -- 为 Agent 栈编写自定义中间件
- [Store](../how-to/store/index.md) -- 支持语义搜索的持久化键值存储
