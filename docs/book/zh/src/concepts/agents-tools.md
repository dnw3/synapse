# Agent 与 Tool

Agent 是由 LLM 决定采取何种行动的系统。与遵循固定脚本不同，模型会审视对话内容，选择调用哪些 Tool（如果需要的话），处理结果，并决定是继续调用更多 Tool 还是给出最终答案。本页解释 Synaptic 如何建模 Tool、如何注册和执行 Tool，以及 Agent 循环的工作原理。

## Tool Trait

Synaptic 中的 Tool 是任何实现了 `Tool` trait 的类型：

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    async fn call(&self, args: Value) -> Result<Value, SynapticError>;
}
```

- `name()` 返回一个唯一标识符，LLM 使用它来引用该 Tool。
- `description()` 用自然语言解释该 Tool 的功能。这会被发送给 LLM，以便它知道何时以及如何使用该 Tool。
- `call()` 使用 JSON 参数执行 Tool 并返回 JSON 结果。

该 trait 设计得非常精简。Tool 不了解对话、记忆或模型。它接收参数，执行工作，返回结果。这使得 Tool 可以被复用，并且可以独立测试。

## ToolDefinition

当 Tool 被发送给 LLM 时，它们以 `ToolDefinition` 结构体的形式描述：

```rust
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub parameters: Value,  // JSON Schema
    pub extras: Option<HashMap<String, Value>>,  // provider-specific params
}
```

`parameters` 字段是一个 JSON Schema，描述了 Tool 期望的参数格式。LLM 提供商使用此 Schema 来生成有效的 Tool 调用。`ToolDefinition` 是关于 Tool 的元数据——它本身不执行任何操作。

可选的 `extras` 字段携带提供商特定的参数（例如 Anthropic 的 `cache_control`）。`synaptic-models` 中的提供商适配器在存在该字段时会将其转发给 API。

## ToolCall 和 ToolChoice

当 LLM 决定使用 Tool 时，它会生成一个 `ToolCall`：

```rust
pub struct ToolCall {
    pub id: String,
    pub name: String,
    pub arguments: Value,
}
```

`id` 将调用与其结果关联起来。当 Tool 执行完成后，结果被封装为 `Message::tool(result, tool_call_id)`，引用该 ID，使得 LLM 能够将结果与调用匹配。

`ToolChoice` 控制 LLM 的 Tool 调用行为：

| 变体 | 行为 |
|---------|----------|
| `Auto` | 模型自行决定是否调用 Tool |
| `Required` | 模型必须至少调用一个 Tool |
| `None` | 禁用 Tool 调用 |
| `Specific(name)` | 模型必须调用指定名称的 Tool |

`ToolChoice` 通过 `.with_tool_choice()` 设置在 `ChatRequest` 上。

## ToolRegistry

`ToolRegistry` 是一个线程安全的 Tool 集合，底层由 `Arc<RwLock<HashMap<String, Arc<dyn Tool>>>>` 支撑：

```rust
use synaptic::tools::ToolRegistry;

let registry = ToolRegistry::new();
registry.register(Arc::new(WeatherTool))?;
registry.register(Arc::new(CalculatorTool))?;

// Look up a tool by name
let tool = registry.get("weather");
```

注册是幂等的——注册同名 Tool 会替换之前的。`Arc<RwLock<_>>` 确保安全的并发访问：多个读者可以同时查找 Tool，而注册操作会短暂获取写锁。

## Tool 执行器

执行器弥合了来自 LLM 的 Tool 调用与 Tool 注册表之间的差距：

**`SerialToolExecutor`** -- 逐一执行 Tool 调用。简单且可预测：

```rust
let executor = SerialToolExecutor::new(registry);
let result = executor.execute("weather", json!({"city": "Tokyo"})).await?;
```

**`ParallelToolExecutor`** -- 并发执行多个 Tool 调用。当 LLM 在单次响应中产生多个独立 Tool 调用时非常有用。

## Tool 包装器

Synaptic 提供了为现有 Tool 添加行为的包装器类型：

- **`HandleErrorTool`** -- 捕获内部 Tool 的错误，并将其作为字符串结果返回，而不是传播错误。这允许 LLM 看到错误并使用不同的参数重试。
- **`ReturnDirectTool`** -- 将 Tool 的输出标记为最终响应，短路 Agent 循环，而不是将结果反馈给 LLM。

## ToolNode

在 Graph 系统中，`ToolNode` 是一个预构建的图节点，用于处理包含 Tool 调用的 AI 消息。它的工作流程：

1. 从 Graph 状态中读取最后一条消息
2. 从中提取所有 `ToolCall` 条目
3. 通过 `SerialToolExecutor` 执行每个 Tool 调用
4. 将结果作为 `Message::tool(...)` 消息追加回状态

`ToolNode` 是在 Graph 工作流中处理 Tool 执行的标准方式。你不需要自己编写 Tool 分发逻辑。

## ReAct Agent 模式

ReAct（推理 + 行动）是最常见的 Agent 模式。模型在推理应该做什么和通过调用 Tool 采取行动之间交替进行。Synaptic 通过 `create_react_agent()` 提供了预构建的 ReAct Agent：

```rust
use synaptic::graph::{create_react_agent, MessageState};

let graph = create_react_agent(model, tools)?;
let state = MessageState::from_messages(vec![
    Message::human("What is the weather in Tokyo?"),
]);
let result = graph.invoke(state).await?;
```

这会构建一个包含两个节点的 Graph：

```
[START] --> [agent] --tool_calls--> [tools] --> [agent] ...
                   \--no_tools----> [END]
```

- **"agent" 节点**：使用当前消息和 Tool 定义调用 LLM。LLM 的响应被追加到状态中。
- **"tools" 节点**：一个 `ToolNode`，执行 Agent 响应中的所有 Tool 调用并追加结果。

"agent" 之后的条件边检查最后一条消息是否包含 Tool 调用。如果有，路由到 "tools"。如果没有，路由到 END。从 "tools" 到 "agent" 的边总是返回，形成循环。

### Agent 循环详解

1. 用户消息进入 Graph 状态。
2. "agent" 节点将所有消息连同 Tool 定义一起发送给 LLM。
3. LLM 做出响应。如果包含 Tool 调用：
   a. 响应（包含 Tool 调用）被追加到状态中。
   b. 路由将执行发送到 "tools" 节点。
   c. 每个 Tool 调用被执行，结果作为 Tool 消息追加。
   d. 路由将执行发送回 "agent" 节点。
   e. LLM 现在可以看到 Tool 结果并决定下一步操作。
4. 当 LLM 响应中不包含 Tool 调用时，表示它已经给出了最终答案。路由将执行发送到 END。

此循环持续进行，直到 LLM 判断它拥有足够的信息可以直接回答，或者达到 Graph 的迭代安全限制（100 次）。

## ReactAgentOptions

`create_react_agent_with_options()` 函数接受一个 `ReactAgentOptions` 结构体用于高级配置：

```rust
let options = ReactAgentOptions {
    checkpointer: Some(Arc::new(MemorySaver::new())),
    system_prompt: Some("You are a helpful weather assistant.".into()),
    interrupt_before: vec!["tools".into()],
    interrupt_after: vec![],
};

let graph = create_react_agent_with_options(model, tools, options)?;
```

| 选项 | 用途 |
|--------|---------|
| `checkpointer` | 状态持久化，支持跨调用恢复 |
| `system_prompt` | 在每次 LLM 调用前追加到消息前面 |
| `interrupt_before` | 在指定节点之前暂停（用于人工审批 Tool 调用） |
| `interrupt_after` | 在指定节点之后暂停（用于人工审查 Tool 结果） |

设置 `interrupt_before: vec!["tools".into()]` 可以创建一个人机交互 Agent：Graph 在执行 Tool 之前暂停，允许人工检查提议的 Tool 调用、修改或完全拒绝它们。然后通过 `update_state()` 恢复 Graph 执行。

## 参见

- [自定义 Tool](../how-to/tools/custom-tool.md) -- 使用 `#[tool]` 宏创建 Tool
- [Tool Registry](../how-to/tools/registry.md) -- 管理 Tool 集合
- [Tool Choice](../how-to/tools/tool-choice.md) -- 控制模型的 Tool 调用行为
- [Tool Definition Extras](../how-to/tools/tool-extras.md) -- 提供商特定参数
- [Runtime-Aware Tools](../how-to/tools/runtime-aware.md) -- 可访问 Store/状态的 Tool
- [Tool Node](../how-to/graph/tool-node.md) -- Graph 工作流中的 ToolNode
- [Graph](graph.md) -- Agent 运行所在的 Graph 系统
