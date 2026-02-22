# Perplexity AI

[Perplexity AI](https://www.perplexity.ai/) 通过 Sonar 模型系列提供联网搜索增强的语言模型服务。与传统 LLM 不同，Sonar 模型能够访问实时网络信息并返回引用来源，非常适合事实性查询和研究任务。

`synaptic-perplexity` crate 封装了 `synaptic-openai`，预设了 Perplexity 的 base URL 并提供类型安全的模型枚举。

## 安装

```toml
[dependencies]
synaptic = { version = "0.2", features = ["perplexity"] }
```

在 [perplexity.ai](https://www.perplexity.ai/) 注册以获取 API 密钥（以 `pplx-` 开头）。

## 配置

```rust,ignore
use synaptic::perplexity::{PerplexityChatModel, PerplexityConfig, PerplexityModel};
use synaptic::models::HttpBackend;
use std::sync::Arc;

let config = PerplexityConfig::new("pplx-your-api-key", PerplexityModel::SonarLarge);
let model = PerplexityChatModel::new(config, Arc::new(HttpBackend::new()));
```

### Builder 方法

```rust,ignore
let config = PerplexityConfig::new("pplx-your-api-key", PerplexityModel::SonarLarge)
    .with_temperature(0.2)
    .with_max_tokens(1024);
```

## 可用模型

| 枚举变体 | API 模型 ID | 适用场景 |
|---|---|---|
| `SonarLarge` | `sonar-large-online` | 通用联网搜索（推荐） |
| `SonarSmall` | `sonar-small-online` | 快速、低成本联网搜索 |
| `SonarHuge` | `sonar-huge-online` | 最高质量联网搜索 |
| `SonarReasoningPro` | `sonar-reasoning-pro` | 带引用的复杂推理 |
| `Custom(String)` | _(任意)_ | 预览模型 |

## 使用示例

```rust,ignore
use synaptic::perplexity::{PerplexityChatModel, PerplexityConfig, PerplexityModel};
use synaptic::core::{ChatModel, ChatRequest, Message};
use synaptic::models::HttpBackend;
use std::sync::Arc;

let config = PerplexityConfig::new("pplx-your-api-key", PerplexityModel::SonarLarge);
let model = PerplexityChatModel::new(config, Arc::new(HttpBackend::new()));

let request = ChatRequest::new(vec![
    Message::system("请精确简洁地回答，并引用来源。"),
    Message::human("Rust 在系统编程中的现状如何？"),
]);

let response = model.chat(request).await?;
println!("{}", response.message.content());
```

## 流式输出

```rust,ignore
use futures::StreamExt;

let mut stream = model.stream_chat(ChatRequest::new(vec![
    Message::human("大语言模型研究的最新进展是什么？"),
]));
while let Some(chunk) = stream.next().await {
    print!("{}", chunk?.content);
}
println!();
```

## 配置参数

| 字段 | 类型 | 默认值 | 说明 |
|-------|------|---------|-------------|
| `api_key` | `String` | 必填 | Perplexity API 密钥（`pplx-...`） |
| `model` | `String` | 枚举决定 | API 模型标识符 |
| `max_tokens` | `Option<u32>` | `None` | 最大生成 token 数 |
| `temperature` | `Option<f64>` | `None` | 采样温度（0.0–2.0） |
| `top_p` | `Option<f64>` | `None` | 核采样阈值 |
| `stop` | `Option<Vec<String>>` | `None` | 停止序列 |
