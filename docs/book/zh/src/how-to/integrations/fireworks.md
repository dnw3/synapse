# Fireworks AI

[Fireworks AI](https://fireworks.ai/) 提供最快的开源模型推理，主流模型首 token 延迟低于 100ms。采用 OpenAI 兼容 API，支持 Llama、DeepSeek、Qwen 等主流开源模型。

`synaptic-fireworks` crate 封装了 `synaptic-openai`，预设了 Fireworks AI 的 base URL 并提供类型安全的模型枚举。

## 安装

```toml
[dependencies]
synaptic = { version = "0.2", features = ["fireworks"] }
```

在 [fireworks.ai](https://fireworks.ai/) 注册以获取 API 密钥（以 `fw-` 开头）。

## 配置

```rust,ignore
use synaptic::fireworks::{FireworksChatModel, FireworksConfig, FireworksModel};
use synaptic::models::HttpBackend;
use std::sync::Arc;

let config = FireworksConfig::new("fw-your-api-key", FireworksModel::Llama3_1_70bInstruct);
let model = FireworksChatModel::new(config, Arc::new(HttpBackend::new()));
```

### Builder 方法

```rust,ignore
let config = FireworksConfig::new("fw-your-api-key", FireworksModel::Llama3_1_70bInstruct)
    .with_temperature(0.7)
    .with_max_tokens(4096)
    .with_top_p(0.95);
```

## 可用模型

| 枚举变体 | API 模型 ID | 适用场景 |
|---|---|---|
| `Llama3_1_70bInstruct` | `accounts/fireworks/models/llama-v3p1-70b-instruct` | 通用（推荐） |
| `Llama3_1_8bInstruct` | `accounts/fireworks/models/llama-v3p1-8b-instruct` | 最快、低成本 |
| `DeepSeekR1` | `accounts/fireworks/models/deepseek-r1` | 推理任务 |
| `Qwen2_5_72bInstruct` | `accounts/fireworks/models/qwen2p5-72b-instruct` | 多语言 |
| `Custom(String)` | _(任意)_ | 未列出/预览模型 |

## 使用示例

```rust,ignore
use synaptic::fireworks::{FireworksChatModel, FireworksConfig, FireworksModel};
use synaptic::core::{ChatModel, ChatRequest, Message};
use synaptic::models::HttpBackend;
use std::sync::Arc;

let config = FireworksConfig::new("fw-your-api-key", FireworksModel::Llama3_1_70bInstruct);
let model = FireworksChatModel::new(config, Arc::new(HttpBackend::new()));

let request = ChatRequest::new(vec![
    Message::system("你是一个有用的助手。"),
    Message::human("解释 Rust 中 async 与多线程的区别。"),
]);

let response = model.chat(request).await?;
println!("{}", response.message.content());
```

## 流式输出

```rust,ignore
use futures::StreamExt;

let mut stream = model.stream_chat(ChatRequest::new(vec![
    Message::human("写一首关于 Rust 编程的俳句。"),
]));
while let Some(chunk) = stream.next().await {
    print!("{}", chunk?.content);
}
println!();
```

## 配置参数

| 字段 | 类型 | 默认值 | 说明 |
|-------|------|---------|-------------|
| `api_key` | `String` | 必填 | Fireworks AI API 密钥（`fw-...`） |
| `model` | `String` | 枚举决定 | API 模型标识符 |
| `max_tokens` | `Option<u32>` | `None` | 最大生成 token 数 |
| `temperature` | `Option<f64>` | `None` | 采样温度（0.0–2.0） |
| `top_p` | `Option<f64>` | `None` | 核采样阈值 |
| `stop` | `Option<Vec<String>>` | `None` | 停止序列 |
