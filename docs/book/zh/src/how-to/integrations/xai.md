# xAI Grok

[xAI](https://x.ai/) 开发了 Grok 系列大语言模型，以实时推理能力和 X（Twitter）数据集成著称。Grok API 与 OpenAI API 兼容。

`synaptic-xai` crate 封装了 `synaptic-openai`，预设了 xAI 的 base URL 并提供类型安全的模型枚举。

## 安装

```toml
[dependencies]
synaptic = { version = "0.2", features = ["xai"] }
```

在 [x.ai](https://x.ai/) 注册以获取 API 密钥。

## 配置

```rust,ignore
use synaptic::xai::{XaiChatModel, XaiConfig, XaiModel};
use synaptic::models::HttpBackend;
use std::sync::Arc;

let config = XaiConfig::new("xai-your-api-key", XaiModel::Grok2Latest);
let model = XaiChatModel::new(config, Arc::new(HttpBackend::new()));
```

### Builder 方法

```rust,ignore
let config = XaiConfig::new("xai-your-api-key", XaiModel::Grok2Latest)
    .with_temperature(0.7)
    .with_max_tokens(8192);
```

## 可用模型

| 枚举变体 | API 模型 ID | 适用场景 |
|---|---|---|
| `Grok2Latest` | `grok-2-latest` | 通用（推荐） |
| `Grok2Mini` | `grok-2-mini` | 快速、低成本 |
| `GrokBeta` | `grok-beta` | 兼容旧版 |
| `Custom(String)` | _(任意)_ | 预览模型 |

## 使用示例

```rust,ignore
use synaptic::xai::{XaiChatModel, XaiConfig, XaiModel};
use synaptic::core::{ChatModel, ChatRequest, Message};
use synaptic::models::HttpBackend;
use std::sync::Arc;

let config = XaiConfig::new("xai-your-api-key", XaiModel::Grok2Latest);
let model = XaiChatModel::new(config, Arc::new(HttpBackend::new()));

let request = ChatRequest::new(vec![
    Message::system("你是 Grok，一个有趣且有用的 AI。"),
    Message::human("今天 AI 领域有什么新进展？"),
]);

let response = model.chat(request).await?;
println!("{}", response.message.content());
```

## 流式输出

```rust,ignore
use futures::StreamExt;

let mut stream = model.stream_chat(ChatRequest::new(vec![
    Message::human("简述今日 AI 趋势。"),
]));
while let Some(chunk) = stream.next().await {
    print!("{}", chunk?.content);
}
println!();
```

## 配置参数

| 字段 | 类型 | 默认值 | 说明 |
|-------|------|---------|-------------|
| `api_key` | `String` | 必填 | xAI API 密钥 |
| `model` | `String` | 枚举决定 | API 模型标识符 |
| `max_tokens` | `Option<u32>` | `None` | 最大生成 token 数 |
| `temperature` | `Option<f64>` | `None` | 采样温度（0.0–2.0） |
| `top_p` | `Option<f64>` | `None` | 核采样阈值 |
| `stop` | `Option<Vec<String>>` | `None` | 停止序列 |
