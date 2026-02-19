# 错误处理

Synaptic 在整个框架中使用单一的错误枚举 `SynapticError`。每个异步函数都返回 `Result<T, SynapticError>`，错误通过 `?` 运算符自然传播。本页介绍错误模型、可用的变体以及错误处理和恢复的模式。

## SynapticError

```rust
#[derive(Debug, Error)]
pub enum SynapticError {
    #[error("prompt error: {0}")]           Prompt(String),
    #[error("model error: {0}")]            Model(String),
    #[error("tool error: {0}")]             Tool(String),
    #[error("tool not found: {0}")]         ToolNotFound(String),
    #[error("memory error: {0}")]           Memory(String),
    #[error("rate limit: {0}")]             RateLimit(String),
    #[error("timeout: {0}")]                Timeout(String),
    #[error("validation error: {0}")]       Validation(String),
    #[error("parsing error: {0}")]          Parsing(String),
    #[error("callback error: {0}")]         Callback(String),
    #[error("max steps exceeded: {max_steps}")]  MaxStepsExceeded { max_steps: usize },
    #[error("embedding error: {0}")]        Embedding(String),
    #[error("vector store error: {0}")]     VectorStore(String),
    #[error("retriever error: {0}")]        Retriever(String),
    #[error("loader error: {0}")]           Loader(String),
    #[error("splitter error: {0}")]         Splitter(String),
    #[error("graph error: {0}")]            Graph(String),
    #[error("cache error: {0}")]            Cache(String),
    #[error("config error: {0}")]           Config(String),
    #[error("mcp error: {0}")]             Mcp(String),
}
```

二十个变体，每个子系统一个。这一设计是有意为之的：

- **全局统一类型**：你无需在错误类型之间转换。任何 crate 中的任何函数都可以返回 `SynapticError`，调用方通过 `?` 直接传播，无需转换。
- **字符串载荷**：大多数变体携带一个 `String` 消息。这使得错误类型保持简洁，避免了嵌套的错误层次结构。消息提供了出错原因的上下文。
- **`thiserror` 派生**：`SynapticError` 通过 `#[error(...)]` 属性自动实现 `std::error::Error` 和 `Display`。

## 变体参考

### 基础设施错误

| 变体 | 出现场景 |
|---------|----------------|
| `Model(String)` | LLM 提供商返回错误、网络故障、响应格式无效 |
| `RateLimit(String)` | 提供商速率限制超限、令牌桶耗尽 |
| `Timeout(String)` | 请求超时 |
| `Config(String)` | 配置无效（缺少 API 密钥、参数错误） |

### 输入/输出错误

| 变体 | 出现场景 |
|---------|----------------|
| `Prompt(String)` | 模板变量缺失、模板语法无效 |
| `Validation(String)` | 输入验证失败（如空消息列表、无效 schema） |
| `Parsing(String)` | 输出解析器无法从 LLM 响应中提取结构化数据 |

### 工具错误

| 变体 | 出现场景 |
|---------|----------------|
| `Tool(String)` | 工具执行失败（网络错误、计算错误等） |
| `ToolNotFound(String)` | 请求的工具名称不在注册表中 |

### 子系统错误

| 变体 | 出现场景 |
|---------|----------------|
| `Memory(String)` | 记忆存储读写失败 |
| `Callback(String)` | 回调处理器抛出错误 |
| `Embedding(String)` | 嵌入 API 失败 |
| `VectorStore(String)` | 向量存储读写失败 |
| `Retriever(String)` | 检索操作失败 |
| `Loader(String)` | 文档加载失败（文件未找到、解析错误） |
| `Splitter(String)` | 文本分割失败 |
| `Cache(String)` | 缓存读写失败 |

### 执行控制错误

| 变体 | 出现场景 |
|---------|----------------|
| `Graph(String)` | 图执行错误（编译、路由、节点缺失） |
| `MaxStepsExceeded { max_steps }` | Agent 循环超过最大迭代次数 |
| `Mcp(String)` | MCP 服务器连接、传输或协议错误 |

## 错误传播

由于 Synaptic 中每个异步函数都返回 `Result<T, SynapticError>`，错误可以自然传播：

```rust
async fn process_query(model: &dyn ChatModel, query: &str) -> Result<String, SynapticError> {
    let messages = vec![Message::human(query)];
    let request = ChatRequest::new(messages);
    let response = model.chat(request).await?;  // Model error propagates
    Ok(response.message.content().to_string())
}
```

在应用代码中无需 `.map_err()` 转换。无论是提供商适配器的 `Model` 错误、工具执行的 `Tool` 错误，还是状态机的 `Graph` 错误，都通过同一个 `Result` 类型流转。

## 重试与回退模式

并非所有错误都是致命的。Synaptic 提供了多种弹性机制：

### RetryChatModel

包装一个 `ChatModel`，在遇到瞬时故障时自动重试：

```rust
use synaptic::models::RetryChatModel;

let robust_model = RetryChatModel::new(model, max_retries, delay);
```

失败时，它会等待并最多重试 `max_retries` 次。这处理了瞬时网络错误和速率限制，应用代码无需自行实现重试逻辑。

### RateLimitedChatModel 和 TokenBucketChatModel

通过限流主动防止速率限制错误：

- `RateLimitedChatModel` 限制每个时间窗口内的请求数。
- `TokenBucketChatModel` 使用令牌桶算法实现平滑限流。

通过在触及提供商限制之前进行限流，这些包装器将潜在的 `RateLimit` 错误转化为可控的延迟。

### RunnableWithFallbacks

当主要可运行组件失败时，尝试替代方案：

```rust
use synaptic::runnables::RunnableWithFallbacks;

let chain = RunnableWithFallbacks::new(
    primary.boxed(),
    vec![fallback_1.boxed(), fallback_2.boxed()],
);
```

如果 `primary` 失败，则使用相同的输入尝试 `fallback_1`。如果也失败，再尝试 `fallback_2`。只有当所有选项都失败时，错误才会传播。

### RunnableRetry

使用可配置的指数退避进行重试：

```rust
use std::time::Duration;
use synaptic::runnables::{RunnableRetry, RetryPolicy};

let retry = RunnableRetry::new(
    flaky_step.boxed(),
    RetryPolicy::default()
        .with_max_attempts(4)
        .with_base_delay(Duration::from_millis(200))
        .with_max_delay(Duration::from_secs(5)),
);
```

延迟在每次尝试后翻倍（200ms、400ms、800ms、...），直到 `max_delay`。你还可以设置 `retry_on` 谓词，仅在特定错误类型时重试。这适用于 LCEL 链中的任何步骤，不仅限于模型调用。

### HandleErrorTool

包装工具，使错误作为字符串结果返回而非传播：

```rust
use synaptic::tools::HandleErrorTool;

let safe_tool = HandleErrorTool::new(risky_tool);
```

当内部工具失败时，错误消息成为工具的输出。LLM 看到错误后可以决定用不同的参数重试或采取不同的方法。这防止了单个工具失败导致整个 agent 循环崩溃。

## 图中断（非错误）

图系统中的人工介入中断**不是**错误。Graph 的 `invoke()` 返回 `GraphResult<S>`，它是 `Complete(state)` 或 `Interrupted(state)`：

```rust
use synaptic::graph::GraphResult;

match graph.invoke(state).await? {
    GraphResult::Complete(final_state) => {
        // Graph finished normally
        handle_result(final_state);
    }
    GraphResult::Interrupted(partial_state) => {
        // Human-in-the-loop: inspect state, get approval, resume
        // The graph has checkpointed its state automatically
    }
}
```

要无论完成状态如何都提取状态，使用 `.into_state()`：

```rust
let state = graph.invoke(initial).await?.into_state();
```

也可以通过节点内部的 `Command::interrupt()` 以编程方式触发中断：

```rust
use synaptic::graph::Command;

// Inside a node's process() method:
Command::interrupt(updated_state)
```

`SynapticError::Graph` 保留给真正的错误：编译失败、节点缺失、路由错误和递归限制超限。

## 匹配错误变体

由于 `SynapticError` 是枚举，你可以对特定变体进行匹配以实现针对性的错误处理：

```rust
match result {
    Ok(value) => use_value(value),
    Err(SynapticError::RateLimit(_)) => {
        // Wait and retry
    }
    Err(SynapticError::ToolNotFound(name)) => {
        // Log the missing tool and continue without it
    }
    Err(SynapticError::Parsing(msg)) => {
        // LLM output was malformed; ask the model to try again
    }
    Err(e) => {
        // All other errors: propagate
        return Err(e);
    }
}
```

这种模式在 agent 循环中特别有用，某些错误可以恢复（模型可以重试），而其他错误则不行（网络中断、API 密钥无效）。

## 参考

- [重试与速率限制](../how-to/chat-models/retry-rate-limit.md) -- 模型错误的自动重试
- [回退链](../how-to/runnables/fallbacks.md) -- 用于错误恢复的回退链
- [中断与恢复](../how-to/graph/interrupt-resume.md) -- 图中断（非错误）
