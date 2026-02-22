# Google Gemini

本指南展示如何使用 Synaptic 接入 Google Generative Language API。`GeminiChatModel` 封装了 Google 的 Generative Language REST API，支持流式输出、工具调用以及所有标准 `ChatModel` 操作。

## 设置

在 `Cargo.toml` 中添加 `gemini` feature：

```toml
[dependencies]
synaptic = { version = "0.2", features = ["gemini"] }
```

### API 密钥

将 Google API 密钥设置为环境变量：

```bash
export GOOGLE_API_KEY="AIza..."
```

密钥在构造 `GeminiConfig` 时传入。与其他提供商不同，API 密钥通过查询参数（`?key=...`）传递，而非放在请求头中。

## 配置

使用 API 密钥和模型名称创建 `GeminiConfig`：

```rust,ignore
use synaptic::gemini::{GeminiConfig, GeminiChatModel};
use synaptic::models::HttpBackend;
use std::sync::Arc;

let config = GeminiConfig::new("AIza...", "gemini-2.0-flash");
let model = GeminiChatModel::new(config, Arc::new(HttpBackend::new()));
```

### 自定义 Base URL

如需使用代理或其他端点：

```rust,ignore
let config = GeminiConfig::new(api_key, "gemini-2.0-flash")
    .with_base_url("https://my-proxy.example.com");
```

### 模型参数

```rust,ignore
let config = GeminiConfig::new(api_key, "gemini-2.0-flash")
    .with_top_p(0.9)
    .with_stop(vec!["END".to_string()]);
```

## 用法

`GeminiChatModel` 实现了 `ChatModel` trait：

```rust,ignore
use synaptic::gemini::{GeminiConfig, GeminiChatModel};
use synaptic::core::{ChatModel, ChatRequest, Message};
use synaptic::models::HttpBackend;
use std::sync::Arc;

let config = GeminiConfig::new(
    std::env::var("GOOGLE_API_KEY").unwrap(),
    "gemini-2.0-flash",
);
let model = GeminiChatModel::new(config, Arc::new(HttpBackend::new()));

let request = ChatRequest::new(vec![
    Message::system("你是一个有用的助手。"),
    Message::human("用一句话解释 Rust 的所有权模型。"),
]);

let response = model.chat(request).await?;
println!("{}", response.message.content().unwrap_or_default());
```

## 流式输出

`GeminiChatModel` 通过 `stream_chat` 方法支持原生 SSE 流式输出。流式端点使用 `streamGenerateContent?alt=sse`：

```rust,ignore
use futures::StreamExt;
use synaptic::core::{ChatModel, ChatRequest, Message};

let request = ChatRequest::new(vec![
    Message::human("写一首关于 Rust 的短诗。"),
]);

let mut stream = model.stream_chat(request);
while let Some(chunk) = stream.next().await {
    let chunk = chunk?;
    if !chunk.content.is_empty() {
        print!("{}", chunk.content);
    }
}
```

## 工具调用

Gemini 模型通过 `functionCall` 和 `functionResponse` 部件（camelCase 格式）支持工具调用。Synaptic 会自动将 `ToolDefinition` 和 `ToolChoice` 映射为 Gemini 格式。

```rust,ignore
use synaptic::core::{ChatModel, ChatRequest, Message, ToolDefinition, ToolChoice};

let tools = vec![ToolDefinition {
    name: "get_weather".into(),
    description: "获取指定城市的当前天气".into(),
    parameters: serde_json::json!({
        "type": "object",
        "properties": {
            "city": { "type": "string", "description": "城市名称" }
        },
        "required": ["city"]
    }),
}];

let request = ChatRequest::new(vec![
    Message::human("东京的天气怎么样？"),
])
.with_tools(tools)
.with_tool_choice(ToolChoice::Auto);

let response = model.chat(request).await?;

// 检查模型是否请求了工具调用
for tc in response.message.tool_calls() {
    println!("工具: {}, 参数: {}", tc.name, tc.arguments);
}
```

`ToolChoice` 各变体与 Gemini `functionCallingConfig` 的对应关系：

| Synaptic | Gemini |
|----------|--------|
| `Auto` | `{"mode": "AUTO"}` |
| `Required` | `{"mode": "ANY"}` |
| `None` | `{"mode": "NONE"}` |
| `Specific(name)` | `{"mode": "ANY", "allowedFunctionNames": ["..."]}` |

## 配置参考

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `api_key` | `String` | 必填 | Google API 密钥 |
| `model` | `String` | 必填 | 模型名称（如 `gemini-2.0-flash`） |
| `base_url` | `String` | `"https://generativelanguage.googleapis.com"` | API 基础 URL |
| `top_p` | `Option<f64>` | `None` | 核采样参数 |
| `stop` | `Option<Vec<String>>` | `None` | 停止序列 |
