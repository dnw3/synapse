# 快速入门

本指南将带你通过三个步骤创建并运行一个 Deep Agent。

## 前置条件

在你的 `Cargo.toml` 中添加所需的 crate：

```toml
[dependencies]
synaptic = { version = "0.2", features = ["deep"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## 第一步：创建 Backend

`backend` 决定了 agent 如何与外部世界交互。在本快速入门中，我们使用 `FilesystemBackend`，它可以读写你机器上的真实文件：

```rust,ignore
use synaptic::deep::backend::FilesystemBackend;
use std::sync::Arc;

let backend = Arc::new(FilesystemBackend::new("/tmp/my-workspace"));
```

如果想在不接触文件系统的情况下进行测试，可以换用 `StateBackend::new()`：

```rust,ignore
use synaptic::deep::backend::StateBackend;

let backend = Arc::new(StateBackend::new());
```

## 第二步：创建 Agent

使用 `create_deep_agent` 配合一个模型和一个 `DeepAgentOptions`。选项结构体有合理的默认值——你只需要提供 `backend`：

```rust,ignore
use synaptic::deep::{create_deep_agent, DeepAgentOptions};
use synaptic::openai::OpenAiChatModel;
use std::sync::Arc;

let model = Arc::new(OpenAiChatModel::new("gpt-4o"));
let options = DeepAgentOptions::new(backend);

let agent = create_deep_agent(model, options)?;
```

`create_deep_agent` 会装配完整的中间件栈（记忆、技能、文件系统、子 agent、摘要、工具调用修补），注册文件系统工具，并编译底层的 ReAct 图。它返回一个 `CompiledGraph<MessageState>`。

## 第三步：运行 Agent

使用你的提示词构建一个 `MessageState` 并调用 `invoke`。Agent 会进行推理、调用工具，最终返回结果：

```rust,ignore
use synaptic::graph::MessageState;
use synaptic::core::Message;

let state = MessageState::with_messages(vec![
    Message::human("Create a file called hello.txt containing 'Hello, world!'"),
]);
let result = agent.invoke(state).await?;
println!("{}", result.into_state().last_message_content());
```

## 底层运行机制

当你调用 `agent.invoke(state)` 时：

1. **记忆加载** -- `DeepMemoryMiddleware` 通过 `backend` 检查是否存在 `AGENTS.md` 文件，并将保存的上下文注入系统提示词中。
2. **技能注入** -- `SkillsMiddleware` 扫描 `.skills/` 目录中的 `SKILL.md` 文件，并将匹配的技能指令添加到系统提示词中。
3. **Agent 循环** -- 底层的 ReAct 图进入"推理-行动-观察"循环。模型看到文件系统工具并决定调用哪些。
4. **工具执行** -- 每个工具调用（例如 `write_file`）通过 `backend` 进行调度。`FilesystemBackend` 执行真实的 I/O 操作；`StateBackend` 在内存映射上操作。
5. **摘要** -- 如果对话超过配置的 token 阈值（默认：128,000 token 的 85%），`DeepSummarizationMiddleware` 会在下一次模型调用之前将较早的消息压缩为摘要。
6. **工具调用修补** -- `PatchToolCallsMiddleware` 在工具调用到达执行器之前修复格式错误的调用。
7. **最终回答** -- 当模型响应中不包含工具调用时，图终止，`invoke` 返回 `GraphResult<MessageState>`。

## 自定义选项

`DeepAgentOptions` 的字段可以在传递给 `create_deep_agent` 之前直接设置：

```rust,ignore
let mut options = DeepAgentOptions::new(backend);
options.system_prompt = Some("You are a Rust expert.".to_string());
options.max_input_tokens = 64_000;
options.enable_subagents = false;

let agent = create_deep_agent(model, options)?;
```

关键默认值：

| 字段 | 默认值 |
|------|--------|
| `max_input_tokens` | 128,000 |
| `summarization_threshold` | 0.85 |
| `eviction_threshold` | 20,000 |
| `max_subagent_depth` | 3 |
| `skills_dir` | `".skills"` |
| `memory_file` | `"AGENTS.md"` |
| `enable_subagents` | `true` |
| `enable_filesystem` | `true` |
| `enable_skills` | `true` |
| `enable_memory` | `true` |

## 完整可运行示例

```rust,ignore
use std::sync::Arc;
use synaptic::core::Message;
use synaptic::deep::{create_deep_agent, DeepAgentOptions, backend::FilesystemBackend};
use synaptic::graph::MessageState;
use synaptic::openai::OpenAiChatModel;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let model = Arc::new(OpenAiChatModel::new("gpt-4o"));
    let backend = Arc::new(FilesystemBackend::new("/tmp/demo"));
    let options = DeepAgentOptions::new(backend);

    let agent = create_deep_agent(model, options)?;

    let state = MessageState::with_messages(vec![
        Message::human("What files are in the current directory?"),
    ]);
    let result = agent.invoke(state).await?;
    println!("{}", result.into_state().last_message_content());
    Ok(())
}
```

## 下一步

- [Backend](backends.md) -- 了解 State、Store 和 Filesystem 三种 `backend`
- [文件系统工具](filesystem-tools.md) -- 查看每个工具的功能
- [自定义](customization.md) -- 使用 `DeepAgentOptions` 调整所有选项
