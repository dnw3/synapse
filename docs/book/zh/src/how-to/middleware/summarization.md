# SummarizationMiddleware

当对话历史超过 token 限制时自动进行摘要。适用于长时间运行的 Agent，在上下文窗口可能溢出的场景中，用简洁的摘要替换较早的消息，同时保留近期消息不变。

## 构造函数

```rust,ignore
use synaptic_middleware::SummarizationMiddleware;

let mw = SummarizationMiddleware::new(
    summarizer_model,   // Arc<dyn ChatModel> -- model used to generate summaries
    4000,               // max_tokens -- threshold that triggers summarization
    |msg: &Message| {   // token_counter -- estimates tokens per message
        msg.content().len() / 4
    },
);
```

**参数说明：**

- `model` -- 用于生成摘要的 ChatModel。可以与 Agent 使用相同的模型，也可以使用更便宜/更快的模型。
- `max_tokens` -- 当估算的总 token 数超过此值时触发摘要。
- `token_counter` -- 一个 `Fn(&Message) -> usize` 函数，用于估算单条消息的 token 数。常用的启发式方法是 `content.len() / 4`。

## 在 `create_agent` 中使用

```rust,ignore
use std::sync::Arc;
use synaptic_graph::{create_agent, AgentOptions};
use synaptic_middleware::SummarizationMiddleware;

let summarizer = Arc::new(OpenAiChatModel::new("gpt-4o-mini"));

let options = AgentOptions {
    middleware: vec![
        Arc::new(SummarizationMiddleware::new(
            summarizer,
            4000,
            |msg| msg.content().len() / 4,
        )),
    ],
    ..Default::default()
};

let graph = create_agent(model, tools, options)?;
```

## 工作原理

- **生命周期钩子：** `before_model`
- 在每次模型调用前，Middleware 对所有消息的估算 token 数求和。
- 如果总数在 `max_tokens` 范围内，不做任何操作。
- 如果总数超过限制，将消息分为两组：
  - **近期消息**：在一半 token 预算内的消息（保持原样）。
  - **较早消息**：发送给摘要模型。
- 摘要模型生成简洁的摘要，以带有 `[Previous conversation summary]` 前缀的系统消息替换较早的消息。
- 然后请求以摘要加近期消息继续，保持在预算范围内。

这种方法逐字保留最近的上下文，同时压缩较早的对话内容，让 Agent 了解之前的对话而不超出上下文限制。

## 示例：节省成本的摘要

使用更便宜的模型进行摘要以降低成本：

```rust,ignore
let agent_model = Arc::new(OpenAiChatModel::new("gpt-4o"));
let cheap_model = Arc::new(OpenAiChatModel::new("gpt-4o-mini"));

let options = AgentOptions {
    middleware: vec![
        Arc::new(SummarizationMiddleware::new(
            cheap_model,
            8000,
            |msg| msg.content().len() / 4,
        )),
    ],
    ..Default::default()
};

let graph = create_agent(agent_model, tools, options)?;
```

## 使用 ScriptedChatModel 进行离线测试

无需 API 密钥即可测试摘要行为：

```rust,ignore
use std::sync::Arc;
use synaptic_core::{ChatResponse, Message};
use synaptic_models::ScriptedChatModel;
use synaptic_middleware::SummarizationMiddleware;
use synaptic_graph::{create_agent, AgentOptions, MessageState};

// Script: summarizer returns a summary, agent responds
let summarizer = Arc::new(ScriptedChatModel::new(vec![
    ChatResponse {
        message: Message::ai("Summary: discussed Rust ownership."),
        usage: None,
    },
]));

let agent_model = Arc::new(ScriptedChatModel::new(vec![
    ChatResponse {
        message: Message::ai("Here's more about lifetimes."),
        usage: None,
    },
]));

let options = AgentOptions {
    middleware: vec![
        Arc::new(SummarizationMiddleware::new(
            summarizer,
            100,  // low threshold for testing
            |msg| msg.content().len() / 4,
        )),
    ],
    ..Default::default()
};

let graph = create_agent(agent_model, vec![], options)?;

// Build a state with enough messages to exceed the threshold
let mut state = MessageState::new();
state.messages.push(Message::human("What is Rust?"));
state.messages.push(Message::ai("Rust is a systems programming language..."));
state.messages.push(Message::human("Tell me about ownership."));
state.messages.push(Message::ai("Ownership is a set of rules that govern memory..."));
state.messages.push(Message::human("And lifetimes?"));

let result = graph.invoke(state).await?.into_state();
```
