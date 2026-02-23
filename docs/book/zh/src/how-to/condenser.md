# 上下文压缩

`Condenser` trait 在消息到达模型之前压缩对话历史，使长时间运行的 Agent 能有效管理上下文窗口。

## Condenser Trait

```rust,ignore
use synaptic::condenser::Condenser;

#[async_trait]
pub trait Condenser: Send + Sync {
    async fn condense(&self, messages: Vec<Message>) -> Result<Vec<Message>, SynapticError>;
}
```

## 内置压缩器

### NoOpCondenser

原样返回消息，不做任何修改。适合作为默认值或占位符。

```rust,ignore
use synaptic::condenser::NoOpCondenser;

let condenser = NoOpCondenser;
let output = condenser.condense(messages).await?;
// output == messages（未修改）
```

### RollingCondenser

保留最近的 N 条消息。默认会保留系统消息。

```rust,ignore
use synaptic::condenser::RollingCondenser;

let condenser = RollingCondenser::new(20)
    .with_preserve_system(true);  // 默认值：true
```

### LlmSummarizingCondenser

使用 LLM 对较旧的消息进行摘要，同时保留最近的消息。

```rust,ignore
use synaptic::condenser::LlmSummarizingCondenser;

let condenser = LlmSummarizingCondenser::new(
    model.clone(),   // Arc<dyn ChatModel>
    4096,            // max_tokens 阈值
    5,               // keep_recent：保留的最近消息数
);
```

当估算的 token 数超过 `max_tokens` 时，较旧的消息会被摘要为一条系统消息，并拼接在最近消息之前。

### TokenBudgetCondenser

使用 `TokenCounter` 将消息裁剪到 token 预算范围内。

```rust,ignore
use synaptic::condenser::TokenBudgetCondenser;
use synaptic::core::HeuristicTokenCounter;

let counter = Arc::new(HeuristicTokenCounter);
let condenser = TokenBudgetCondenser::new(4096, counter)
    .with_include_system(true);  // 保留系统消息（默认）
```

### PipelineCondenser

将多个压缩器串联执行。每个压缩器的输出作为下一个的输入。

```rust,ignore
use synaptic::condenser::{PipelineCondenser, RollingCondenser, TokenBudgetCondenser};

let pipeline = PipelineCondenser::new(vec![
    Arc::new(RollingCondenser::new(50)),
    Arc::new(TokenBudgetCondenser::new(4096, counter)),
]);
```

## CondenserMiddleware

将任意压缩器包装为 `AgentMiddleware`，在每次模型调用前自动压缩消息。

```rust,ignore
use synaptic::condenser::CondenserMiddleware;
use synaptic::condenser::RollingCondenser;

let middleware = CondenserMiddleware::new(
    Arc::new(RollingCondenser::new(20)),
);

// 配合 Agent 选项使用
let options = AgentOptions {
    middleware: vec![Arc::new(middleware)],
    ..Default::default()
};
```
