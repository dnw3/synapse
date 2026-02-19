# 子代理

Deep Agent 可以生成子代理（**subagent**）来处理独立的子任务。子代理在自己的上下文中运行，拥有独立的对话历史，并在完成后将结果返回给父代理。

## Task 工具

当子代理功能启用时，`create_deep_agent` 会添加一个内置的 **task** 工具。当父代理调用 `task` 工具时，系统通过 `create_deep_agent()` 创建一个新的子 Deep Agent（使用相同的模型和后端），运行请求的子任务，并将最终回答作为工具结果返回。

```rust,ignore
use std::sync::Arc;
use synaptic_deep::{create_deep_agent, DeepAgentOptions};

let mut options = DeepAgentOptions::new(backend);
options.enable_subagents = true; // 默认启用
let agent = create_deep_agent(model, options)?;

// 代理现在可以在推理循环中调用 "task" 工具。
// 模型可能发出的工具调用示例：
// { "name": "task", "arguments": { "description": "Refactor the parse module" } }
```

`task` 工具接受两个参数：

| 参数 | 是否必需 | 描述 |
|------|----------|------|
| `description` | 是 | 为子代理提供的任务详细描述 |
| `agent_type` | 否 | 要生成的自定义子代理类型名称（默认为 `"general-purpose"`） |

## SubAgentDef

如需更精细的控制，可以使用 `SubAgentDef` 定义命名的子代理类型。每个定义指定名称、描述、系统提示词和可选的工具集。`SubAgentDef` 是一个普通结构体，直接使用结构体字面量创建：

```rust,ignore
use std::sync::Arc;
use synaptic_deep::{create_deep_agent, DeepAgentOptions, SubAgentDef};

let mut options = DeepAgentOptions::new(backend);
options.subagents = vec![
    SubAgentDef {
        name: "researcher".to_string(),
        description: "Research specialist".to_string(),
        system_prompt: "You are a research assistant. Find relevant files and summarize them.".to_string(),
        tools: vec![], // 继承默认的 Deep Agent 工具
    },
    SubAgentDef {
        name: "writer".to_string(),
        description: "Code writer".to_string(),
        system_prompt: "You are a code writer. Implement the requested changes.".to_string(),
        tools: vec![],
    },
];
let agent = create_deep_agent(model, options)?;
```

当父代理使用 `"agent_type": "researcher"` 调用 `task` 工具时，`TaskTool` 会按名称查找匹配的 `SubAgentDef`，并使用其 `system_prompt` 和 `tools` 配置子代理。如果找不到匹配的定义，则使用默认设置生成通用子代理。

## 递归深度控制

子代理本身也可以进一步生成子代理。为防止无限递归，可以配置 `max_subagent_depth`：

```rust,ignore
use std::sync::Arc;
use synaptic_deep::{create_deep_agent, DeepAgentOptions};

let mut options = DeepAgentOptions::new(backend);
options.max_subagent_depth = 3; // 默认值为 3
let agent = create_deep_agent(model, options)?;
```

`SubAgentMiddleware` 使用 `AtomicUsize` 计数器跟踪当前深度。当达到深度限制时，`task` 工具会返回错误而不是生成新代理。父代理将此错误视为工具结果，可以据此调整策略。

## 上下文隔离

每个子代理都从全新的对话开始。父代理的消息历史**不会**被传递。这使子代理保持专注，避免上下文窗口溢出。子代理接收到的信息仅包括：

1. 自身的系统提示词（来自 `SubAgentDef` 或默认的 Deep Agent 提示词）。
2. 父代理提供的任务描述，以 `Message::human()` 形式发送。
3. 共享的后端 -- 子代理可以读写相同的工作区。

子代理是通过 `create_deep_agent()` 创建的完整 Deep Agent，因此它可以访问与父代理相同的文件系统工具、技能和中间件栈（进一步生成子代理受深度限制约束）。

当子代理完成时，只有其最后一条 AI 消息的内容作为工具结果字符串返回给父代理。中间的推理过程和工具调用会被丢弃。

## 示例：委派研究任务

```rust,ignore
use std::sync::Arc;
use synaptic_core::Message;
use synaptic_deep::{create_deep_agent, DeepAgentOptions};
use synaptic_graph::MessageState;

let options = DeepAgentOptions::new(backend);
let agent = create_deep_agent(model, options)?;

let state = MessageState::with_messages(vec![
    Message::human("Find all TODO comments in the codebase and write a summary to TODO_REPORT.md"),
]);
let result = agent.invoke(state).await?;
let final_state = result.into_state();

// 在底层，代理可能会调用：
//   task({ "description": "Search for TODO comments in all .rs files" })
// 子代理运行完毕后返回结果，父代理撰写报告。
```
