# 记忆

Deep Agent 可以通过在工作区中读写记忆文件（默认为 `AGENTS.md`）来跨会话持久化学到的上下文。这为代理提供了一种在重启后仍然有效的长期记忆。

## 工作原理

`DeepMemoryMiddleware` 实现了 `AgentMiddleware`。在每次模型调用时，其 `before_model()` 钩子会从后端读取配置的记忆文件。如果文件存在且非空，其内容会被包裹在 `<agent_memory>` 标签中并追加到系统提示词：

```text
<agent_memory>
- The user prefers tabs over spaces.
- The project uses `thiserror 2.0` for error types.
- Always run `cargo fmt` after editing Rust files.
</agent_memory>
```

如果文件不存在或为空，中间件会静默跳过注入。代理在处理每条消息之前都能看到此上下文，因此可以立即应用学到的偏好。

## 写入记忆

代理可以随时使用内置文件系统工具（如 `write_file` 或 `edit_file`）写入记忆文件来更新记忆。典型的模式是当代理学到重要内容时追加新行：

```text
Agent reasoning: "The user corrected me -- they want snake_case, not camelCase.
I should remember this for future sessions."

Tool call: edit_file({
  "path": "AGENTS.md",
  "old_string": "- Always run `cargo fmt` after editing Rust files.",
  "new_string": "- Always run `cargo fmt` after editing Rust files.\n- Use snake_case for all function names."
})
```

因为中间件在每次模型调用时都会重新读取文件，所以更新会在下一轮对话中立即生效。

## 配置

记忆由 `DeepAgentOptions` 上的两个字段控制：

```rust,ignore
use std::sync::Arc;
use synaptic_deep::{create_deep_agent, DeepAgentOptions};

let mut options = DeepAgentOptions::new(backend.clone());
options.memory_file = Some("AGENTS.md".to_string()); // 默认值
options.enable_memory = true;                         // 默认值

let agent = create_deep_agent(model, options)?;
```

- **`memory_file`**（`Option<String>`，默认 `Some("AGENTS.md")`）-- 后端中记忆文件的路径。你可以将其指向其他文件：

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());
options.memory_file = Some("docs/MEMORY.md".to_string());
```

- **`enable_memory`**（`bool`，默认 `true`）-- 为 `true` 时，`DeepMemoryMiddleware` 会被添加到中间件栈中。

## 禁用记忆

要在不使用持久记忆的情况下运行，将 `enable_memory` 设置为 `false`：

```rust,ignore
let mut options = DeepAgentOptions::new(backend.clone());
options.enable_memory = false;

let agent = create_deep_agent(model, options)?;
```

`DeepMemoryMiddleware` 完全不会被添加到栈中，因此没有任何开销。

## DeepMemoryMiddleware 内部实现

中间件的结构体非常简洁：

```rust,ignore
pub struct DeepMemoryMiddleware {
    backend: Arc<dyn Backend>,
    memory_file: String,
}

impl DeepMemoryMiddleware {
    pub fn new(backend: Arc<dyn Backend>, memory_file: String) -> Self;
}
```

它实现了 `AgentMiddleware`，只有一个钩子：

- **`before_model()`** -- 从后端读取记忆文件。如果内容非空，将其包裹在 `<agent_memory>` 标签中并追加到系统提示词。如果文件缺失或为空，则不做任何操作。

## 中间件栈位置

`DeepMemoryMiddleware` 在中间件栈中最先运行（第 1 位，共 7 个位置），确保记忆上下文对所有后续中间件和模型本身都可用。参见[自定义](customization.md)页面了解完整的组装顺序。
