# Deep Agent

Deep Agent 是一种高级 agent 抽象，它将中间件栈、用于文件系统和状态操作的 `backend`，以及一个一键创建完整配置 agent 的工厂函数组合在一起。它专为需要读写文件、启动子 agent、加载技能以及维护持久化记忆的任务而设计——这些工作流通常出现在编程助手和自主研究 agent 中。

## 架构

Deep Agent 由多个层组装而成，包裹在核心的 ReAct agent 图之上：

```text
+-----------------------------------------------+
|              Deep Agent                        |
|  +------------------------------------------+ |
|  |  Middleware Stack                         | |
|  |  - DeepMemoryMiddleware (AGENTS.md)      | |
|  |  - SkillsMiddleware (SKILL.md injection) | |
|  |  - FilesystemMiddleware (tool eviction)  | |
|  |  - SubAgentMiddleware (task tool)        | |
|  |  - DeepSummarizationMiddleware           | |
|  |  - PatchToolCallsMiddleware              | |
|  +------------------------------------------+ |
|  +------------------------------------------+ |
|  |  Filesystem Tools                         | |
|  |  ls, read_file, write_file, edit_file,    | |
|  |  glob, grep (+execute if supported)       | |
|  +------------------------------------------+ |
|  +------------------------------------------+ |
|  |  Backend (State / Store / Filesystem)     | |
|  +------------------------------------------+ |
|  +------------------------------------------+ |
|  |  ReAct Agent Graph (agent + tools nodes)  | |
|  +------------------------------------------+ |
+-----------------------------------------------+
```

## 核心能力

| 能力 | 描述 |
|------|------|
| 文件系统工具 | 通过可插拔的 `backend` 读取、写入、编辑、搜索和列出文件。当 `backend` 支持时会自动添加 `execute` 工具。 |
| 子 agent | 启动子 agent 来执行隔离的子任务，支持递归深度控制（`max_subagent_depth`） |
| 技能 | 从可配置的目录加载 `SKILL.md` 文件，将特定领域的指令注入系统提示词中 |
| 记忆 | 在 `AGENTS.md` 中持久化已学习的上下文，并在跨会话时重新加载 |
| 摘要 | 当上下文长度超过 `max_input_tokens` 的 `summarization_threshold` 比例时，自动对对话历史进行摘要 |
| Backend 抽象 | 在内存（`StateBackend`）、持久化存储（`StoreBackend`）和真实文件系统（`FilesystemBackend`）三种 `backend` 之间自由切换 |

## 最简示例

```rust,ignore
use synaptic_deep::{create_deep_agent, DeepAgentOptions, backend::FilesystemBackend};
use synaptic_graph::MessageState;
use synaptic_models::OpenAiChatModel;
use synaptic_core::Message;
use std::sync::Arc;

let model = Arc::new(OpenAiChatModel::new("gpt-4o"));
let backend = Arc::new(FilesystemBackend::new("/path/to/workspace"));
let options = DeepAgentOptions::new(backend);

let agent = create_deep_agent(model, options)?;

let result = agent.invoke(MessageState::with_messages(vec![
    Message::human("List the Rust files in src/"),
])).await?;
println!("{}", result.into_state().last_message_content());
```

`create_deep_agent` 返回一个 `CompiledGraph<MessageState>` —— 与 `create_react_agent` 使用的图类型相同。你传入包含输入消息的 `MessageState` 来调用它，并收到一个 `GraphResult<MessageState>` 作为返回值。

## 指南

- [快速入门](quickstart.md) -- 创建并运行你的第一个 Deep Agent
- [Backend](backends.md) -- 在 State、Store 和 Filesystem 三种 `backend` 之间做出选择
- [文件系统工具](filesystem-tools.md) -- 内置工具参考
- [子 Agent](subagents.md) -- 将子任务委托给子 agent
- [技能](skills.md) -- 使用 SKILL.md 文件扩展 agent 行为
- [记忆](memory.md) -- 通过 AGENTS.md 实现持久化 agent 记忆
- [自定义](customization.md) -- 完整的 `DeepAgentOptions` 参考

## 何时使用 Deep Agent

当你的任务涉及**文件操作**、**基于项目状态的多步推理**或**启动子任务**时，请使用 Deep Agent。如果你只需要一个简单的问答循环，普通的 `create_react_agent` 就足够了。Deep Agent 增加了基础设施层，将基本的 ReAct 循环转变为自主编程或研究助手。
