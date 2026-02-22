# Ollama

本指南展示如何使用 [Ollama](https://ollama.com/) 作为 Synaptic 的本地聊天模型和嵌入向量提供者。`OllamaChatModel` 封装了 Ollama REST API，支持流式输出、工具调用以及所有标准 `ChatModel` 操作。由于 Ollama 在本地运行，无需 API 密钥。

## 设置

在 `Cargo.toml` 中添加 `ollama` feature：

```toml
[dependencies]
synaptic = { version = "0.2", features = ["ollama"] }
```

### 安装 Ollama

从 [ollama.com](https://ollama.com/) 安装 Ollama，并在使用前拉取模型：

```bash
# 安装 Ollama（macOS）
brew install ollama

# 启动 Ollama 服务
ollama serve

# 拉取模型
ollama pull llama3.1
```

默认端点为 `http://localhost:11434`。发送请求前请确保 Ollama 服务已启动。

## 配置

使用模型名称创建 `OllamaConfig`，无需 API 密钥：

```rust,ignore
use synaptic::ollama::{OllamaConfig, OllamaChatModel};
use synaptic::models::HttpBackend;
use std::sync::Arc;

let config = OllamaConfig::new("llama3.1");
let model = OllamaChatModel::new(config, Arc::new(HttpBackend::new()));
```

### 自定义 Base URL

连接远程 Ollama 实例或非默认端口：

```rust,ignore
let config = OllamaConfig::new("llama3.1")
    .with_base_url("http://192.168.1.100:11434");
```

### 模型参数

```rust,ignore
let config = OllamaConfig::new("llama3.1")
    .with_top_p(0.9)
    .with_stop(vec!["END".to_string()])
    .with_seed(42);
```

## 用法

`OllamaChatModel` 实现了 `ChatModel` trait：

```rust,ignore
use synaptic::ollama::{OllamaConfig, OllamaChatModel};
use synaptic::core::{ChatModel, ChatRequest, Message};
use synaptic::models::HttpBackend;
use std::sync::Arc;

let config = OllamaConfig::new("llama3.1");
let model = OllamaChatModel::new(config, Arc::new(HttpBackend::new()));

let request = ChatRequest::new(vec![
    Message::system("你是一个有用的助手。"),
    Message::human("用一句话解释 Rust 的所有权模型。"),
]);

let response = model.chat(request).await?;
println!("{}", response.message.content().unwrap_or_default());
```

## 流式输出

`OllamaChatModel` 通过 `stream_chat` 方法支持原生流式输出。与使用 SSE 的云服务商不同，Ollama 使用 NDJSON（换行符分隔的 JSON）格式，每行是一个完整的 JSON 对象：

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

支持函数调用的 Ollama 模型（如 `llama3.1`）可以通过 `tool_calls` 数组格式使用工具调用。Synaptic 会自动将 `ToolDefinition` 和 `ToolChoice` 映射为 Ollama 格式。

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

`ToolChoice` 各变体与 Ollama `tool_choice` 的对应关系：

| Synaptic | Ollama |
|----------|--------|
| `Auto` | `"auto"` |
| `Required` | `"required"` |
| `None` | `"none"` |
| `Specific(name)` | `{"type": "function", "function": {"name": "..."}}` |

## 使用 seed 实现可复现生成

Ollama 支持 `seed` 参数来实现可复现的生成。设置后，相同输入下模型将产生确定性输出：

```rust,ignore
let config = OllamaConfig::new("llama3.1")
    .with_seed(42);
let model = OllamaChatModel::new(config, Arc::new(HttpBackend::new()));

let request = ChatRequest::new(vec![
    Message::human("在 1 到 100 之间随机选一个数字。"),
]);

// 相同 seed + 相同输入 = 相同输出
let response = model.chat(request).await?;
println!("{}", response.message.content().unwrap_or_default());
```

## 嵌入向量

`OllamaEmbeddings` 通过 Ollama 的 `/api/embed` 端点提供本地嵌入向量生成。需先拉取嵌入模型：

```bash
ollama pull nomic-embed-text
```

### 配置

```rust,ignore
use synaptic::ollama::{OllamaEmbeddingsConfig, OllamaEmbeddings};
use synaptic::models::HttpBackend;
use std::sync::Arc;

let config = OllamaEmbeddingsConfig::new("nomic-embed-text");
let embeddings = OllamaEmbeddings::new(config, Arc::new(HttpBackend::new()));
```

连接远程实例：

```rust,ignore
let config = OllamaEmbeddingsConfig::new("nomic-embed-text")
    .with_base_url("http://192.168.1.100:11434");
```

### 用法

`OllamaEmbeddings` 实现了 `Embeddings` trait：

```rust,ignore
use synaptic::core::Embeddings;

// 嵌入单个查询
let vector = embeddings.embed_query("什么是 Rust？").await?;
println!("维度: {}", vector.len());

// 嵌入多个文档
let vectors = embeddings.embed_documents(&["第一篇文档", "第二篇文档"]).await?;
println!("已嵌入 {} 篇文档", vectors.len());
```

## 配置参考

### OllamaConfig

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `model` | `String` | 必填 | 模型名称（如 `llama3.1`） |
| `base_url` | `String` | `"http://localhost:11434"` | Ollama 服务地址 |
| `top_p` | `Option<f64>` | `None` | 核采样参数 |
| `stop` | `Option<Vec<String>>` | `None` | 停止序列 |
| `seed` | `Option<u64>` | `None` | 可复现生成的随机种子 |

### OllamaEmbeddingsConfig

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `model` | `String` | 必填 | 嵌入模型名称（如 `nomic-embed-text`） |
| `base_url` | `String` | `"http://localhost:11434"` | Ollama 服务地址 |
