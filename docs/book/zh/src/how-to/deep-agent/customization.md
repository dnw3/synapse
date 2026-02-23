# 自定义

Deep Agent 的每个方面都可以通过 `DeepAgentOptions` 进行调整。本页是逐字段的参考手册，附带示例。

## DeepAgentOptions 参考

`DeepAgentOptions` 使用直接字段赋值而非构建器模式。使用 `DeepAgentOptions::new(backend)` 创建一个具有合理默认值的实例，然后根据需要覆盖字段：

```rust,ignore
use std::sync::Arc;
use synaptic::deep::{create_deep_agent, DeepAgentOptions};

let mut options = DeepAgentOptions::new(backend.clone());
options.system_prompt = Some("You are a senior Rust engineer.".into());
options.max_subagent_depth = 2;

let agent = create_deep_agent(model, options)?;
```

### 完整字段列表

```rust,ignore
pub struct DeepAgentOptions {
    pub backend: Arc<dyn Backend>,                    // 必需
    pub system_prompt: Option<String>,                // None
    pub tools: Vec<Arc<dyn Tool>>,                    // 空
    pub middleware: Vec<Arc<dyn AgentMiddleware>>,     // 空
    pub checkpointer: Option<Arc<dyn Checkpointer>>,  // None
    pub store: Option<Arc<dyn Store>>,                // None
    pub max_input_tokens: usize,                      // 128_000
    pub summarization_threshold: f64,                  // 0.85
    pub eviction_threshold: usize,                     // 20_000
    pub max_subagent_depth: usize,                     // 3
    pub skills_dir: Option<String>,                    // Some(".skills")
    pub memory_file: Option<String>,                   // Some("AGENTS.md")
    pub subagents: Vec<SubAgentDef>,                   // 空
    pub enable_subagents: bool,                        // true
    pub enable_filesystem: bool,                       // true
    pub enable_skills: bool,                           // true
    pub enable_memory: bool,                           // true
}
```

## 字段详解

### backend

后端为代理提供文件系统操作。这是 `DeepAgentOptions::new()` 唯一的必需参数。所有其他字段都有默认值。

```rust,ignore
use synaptic::deep::backend::FilesystemBackend;

let backend = Arc::new(FilesystemBackend::new("/home/user/project"));
let options = DeepAgentOptions::new(backend);
```

### system_prompt

完全覆盖默认的系统提示词。当值为 `None` 时，代理使用描述文件系统工具和预期行为的内置提示词。

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());
options.system_prompt = Some("You are a Rust expert. Use the provided tools to help.".into());
```

### tools

除内置文件系统工具之外的额外工具。这些工具会被添加到代理的工具注册表中，并对模型可用。

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());
options.tools = vec![
    Arc::new(MyCustomTool),
    Arc::new(DatabaseQueryTool::new(db_pool)),
];
```

### middleware

在整个内置栈之后运行的自定义中间件层。详见[中间件栈](#中间件栈)了解顺序细节。

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());
options.middleware = vec![
    Arc::new(AuditLogMiddleware::new(log_file)),
];
```

### checkpointer

可选的检查点器，用于图状态持久化。提供后，代理可以从检查点恢复。

```rust,ignore
use synaptic::graph::StoreCheckpointer;
use synaptic::store::InMemoryStore;

let mut options = DeepAgentOptions::new(backend.clone());
options.checkpointer = Some(Arc::new(StoreCheckpointer::new(Arc::new(InMemoryStore::new()))));
```

### store

可选的存储，通过 `ToolRuntime` 用于运行时工具注入。

```rust,ignore
use synaptic::store::InMemoryStore;

let mut options = DeepAgentOptions::new(backend.clone());
options.store = Some(Arc::new(InMemoryStore::new()));
```

### max_input_tokens

触发摘要前的最大输入 token 数（默认 `128_000`）。`DeepSummarizationMiddleware` 将此值与 `summarization_threshold` 配合使用，决定何时压缩上下文。

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());
options.max_input_tokens = 200_000; // 适用于上下文窗口更大的模型
```

### summarization_threshold

触发摘要的 `max_input_tokens` 比例（默认 `0.85`）。当上下文超过 `max_input_tokens * summarization_threshold` 个 token 时，中间件会对较早的消息进行摘要。

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());
options.summarization_threshold = 0.70; // 更早触发摘要
```

### eviction_threshold

超过此 token 数量时，`FilesystemMiddleware` 会将工具结果驱逐到文件（默认 `20_000`）。较大的工具输出会被写入文件，并替换为引用。

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());
options.eviction_threshold = 10_000; // 驱逐更小的结果
```

### max_subagent_depth

嵌套子代理生成的最大递归深度（默认 `3`）。防止代理链失控。

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());
options.max_subagent_depth = 2;
```

### skills_dir

后端中扫描技能文件的目录路径（默认 `Some(".skills")`）。设置为 `None` 可禁用技能扫描，即使 `enable_skills` 为 true 也是如此。

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());
options.skills_dir = Some("my-skills".into());
```

### memory_file

后端中持久记忆文件的路径（默认 `Some("AGENTS.md")`）。详见[记忆](memory.md)页面。

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());
options.memory_file = Some("docs/MEMORY.md".into());
```

### subagents

`task` 工具的自定义子代理定义。每个 `SubAgentDef` 描述一个可以被生成的专用子代理。

```rust,ignore
use synaptic::deep::SubAgentDef;

let mut options = DeepAgentOptions::new(backend.clone());
options.subagents = vec![
    SubAgentDef {
        name: "researcher".into(),
        description: "Searches the web for information".into(),
        // ...
    },
];
```

### enable_subagents

切换用于子代理生成的 `task` 工具（默认 `true`）。为 `false` 时，`SubAgentMiddleware` 及其 `task` 工具不会被添加。

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());
options.enable_subagents = false;
```

### enable_filesystem

切换内置文件系统工具和 `FilesystemMiddleware`（默认 `true`）。为 `false` 时，不会注册任何文件系统工具。

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());
options.enable_filesystem = false;
```

### enable_skills

切换用于渐进式技能披露的 `SkillsMiddleware`（默认 `true`）。

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());
options.enable_skills = false;
```

### enable_memory

切换用于持久记忆的 `DeepMemoryMiddleware`（默认 `true`）。详见[记忆](memory.md)页面。

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());
options.enable_memory = false;
```

## 中间件栈

`create_deep_agent` 按固定顺序组装中间件栈。每个层可以单独启用或禁用：

| 顺序 | 中间件 | 控制方式 |
|------|--------|----------|
| 1 | `DeepMemoryMiddleware` | `enable_memory` |
| 2 | `SkillsMiddleware` | `enable_skills` |
| 3 | `FilesystemMiddleware` + 文件系统工具 | `enable_filesystem` |
| 4 | `SubAgentMiddleware` 的 `task` 工具 | `enable_subagents` |
| 5 | `DeepSummarizationMiddleware` | 始终添加 |
| 6 | `PatchToolCallsMiddleware` | 始终添加 |
| 7 | 用户提供的中间件 | `middleware` 字段 |

`DeepSummarizationMiddleware` 和 `PatchToolCallsMiddleware` 始终存在，不受配置影响。

## 返回类型

`create_deep_agent` 返回 `Result<CompiledGraph<MessageState>, SynapticError>`。生成的图与任何其他 Synaptic 图的使用方式相同：

```rust,ignore
use synaptic::core::Message;
use synaptic::graph::MessageState;

let agent = create_deep_agent(model, options)?;
let result = agent.invoke(MessageState::with_messages(vec![
    Message::human("Refactor the error handling in src/lib.rs"),
])).await?;
```

## 完整示例

```rust,ignore
use std::sync::Arc;
use synaptic::core::Message;
use synaptic::deep::{create_deep_agent, DeepAgentOptions, backend::FilesystemBackend};
use synaptic::graph::MessageState;
use synaptic::openai::OpenAiChatModel;

let model = Arc::new(OpenAiChatModel::new("gpt-4o"));
let backend = Arc::new(FilesystemBackend::new("/home/user/project"));

let mut options = DeepAgentOptions::new(backend);
options.system_prompt = Some("You are a senior Rust engineer.".into());
options.summarization_threshold = 0.70;
options.enable_subagents = true;
options.max_subagent_depth = 2;

let agent = create_deep_agent(model, options)?;
let result = agent.invoke(MessageState::with_messages(vec![
    Message::human("Refactor the error handling in src/lib.rs"),
])).await?;
```
