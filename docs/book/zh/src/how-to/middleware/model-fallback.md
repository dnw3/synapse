# ModelFallbackMiddleware

当主模型失败时回退到备用模型。适用于需要无缝故障转移的高可用场景，例如在不同提供商之间切换（如 OpenAI 到 Anthropic）或在不同模型层级之间切换（如 GPT-4 到 GPT-3.5）。

## 构造函数

```rust,ignore
use synaptic::middleware::ModelFallbackMiddleware;

let mw = ModelFallbackMiddleware::new(vec![
    fallback_model_1,  // Arc<dyn ChatModel>
    fallback_model_2,  // Arc<dyn ChatModel>
]);
```

备用模型列表按顺序尝试。返回第一个成功的响应。

## 在 `create_agent` 中使用

```rust,ignore
use std::sync::Arc;
use synaptic::graph::{create_agent, AgentOptions};
use synaptic::openai::OpenAiChatModel;
use synaptic::anthropic::AnthropicChatModel;
use synaptic::middleware::ModelFallbackMiddleware;

let primary = Arc::new(OpenAiChatModel::new("gpt-4o"));
let fallback = Arc::new(AnthropicChatModel::new("claude-sonnet-4-20250514"));

let options = AgentOptions {
    middleware: vec![
        Arc::new(ModelFallbackMiddleware::new(vec![fallback])),
    ],
    ..Default::default()
};

let graph = create_agent(primary, tools, options)?;
```

## 工作原理

- **生命周期钩子：** `wrap_model_call`
- Middleware 首先委托给 `next.call(request)`，通过 Middleware 链的其余部分调用主模型。
- 如果主模型调用成功，直接返回响应。
- 如果主模型调用失败，Middleware 按顺序尝试每个备用模型，通过创建 `BaseChatModelCaller` 发送相同的请求。
- 返回第一个成功的备用模型响应。如果所有备用模型也失败，返回主模型的原始错误。

备用模型直接调用（绕过 Middleware 链），以避免其他 Middleware 可能导致或加剧的故障的干扰。

## 示例：多层级回退

```rust,ignore
let primary: Arc<dyn ChatModel> = Arc::new(OpenAiChatModel::new("gpt-4o"));
let tier2: Arc<dyn ChatModel> = Arc::new(OpenAiChatModel::new("gpt-4o-mini"));
let tier3: Arc<dyn ChatModel> = Arc::new(AnthropicChatModel::new("claude-sonnet-4-20250514"));

let options = AgentOptions {
    middleware: vec![
        Arc::new(ModelFallbackMiddleware::new(vec![tier2, tier3])),
    ],
    ..Default::default()
};

let graph = create_agent(primary, tools, options)?;
```

Agent 首先尝试 GPT-4o，然后是 GPT-4o-mini，最后是 Claude Sonnet。
