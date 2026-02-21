# Azure OpenAI

本指南展示如何使用 Synaptic 接入 [Azure OpenAI Service](https://azure.microsoft.com/products/ai-services/openai-service)。Azure OpenAI 提供与 OpenAI 相同的模型，但通过 Azure 的企业级基础设施进行部署和管理。

## 设置

在 `Cargo.toml` 中添加 `openai` feature：

```toml
[dependencies]
synaptic = { version = "0.2", features = ["openai"] }
```

Azure OpenAI 使用 `synaptic-openai` crate 中的 Azure 专用模块，因此只需启用 `openai` feature。

你需要在 Azure 门户中创建一个 Azure OpenAI 资源，并部署一个模型。你将需要以下信息：

- **API Key** -- Azure 门户中的密钥
- **Resource Name** -- 你的 Azure OpenAI 资源名称
- **Deployment Name** -- 你部署的模型名称

## 配置

使用 `AzureOpenAiConfig` 创建配置：

```rust,ignore
use synaptic::openai::{AzureOpenAiConfig, AzureOpenAiChatModel};

let config = AzureOpenAiConfig::new(
    "your-api-key",
    "your-resource-name",
    "your-deployment-name",
);
```

配置会自动构建以下 URL 格式：

```text
https://{resource_name}.openai.azure.com/openai/deployments/{deployment_name}
```

### API 版本

默认 API 版本为 `"2024-10-21"`。如需使用其他版本：

```rust,ignore
let config = AzureOpenAiConfig::new("key", "resource", "deployment")
    .with_api_version("2024-12-01-preview");
```

### 自定义 Base URL

如果你使用自定义端点（例如代理或私有链接），可以覆盖 base URL：

```rust,ignore
let config = AzureOpenAiConfig::new("key", "resource", "deployment")
    .with_base_url("https://my-proxy.example.com/openai");
```

## 用法

### Chat Model

```rust,ignore
use std::sync::Arc;
use synaptic::openai::{AzureOpenAiConfig, AzureOpenAiChatModel};
use synaptic::models::HttpBackend;
use synaptic::core::{ChatModel, ChatRequest, Message};

let config = AzureOpenAiConfig::new(
    "your-api-key",
    "my-resource",
    "gpt-4o",
);

let model = AzureOpenAiChatModel::new(config, Arc::new(HttpBackend::new()));

let request = ChatRequest::new(vec![Message::human("你好！")]);
let response = model.chat(&request).await?;
println!("{}", response.message.content().unwrap_or_default());
```

Azure OpenAI 使用 `api-key` HTTP header 进行认证，而非 OpenAI 使用的 `Authorization: Bearer` 方式。此差异由 `AzureOpenAiChatModel` 自动处理。

### 流式输出

```rust,ignore
use futures::StreamExt;
use synaptic::core::{ChatModel, ChatRequest, Message};

let request = ChatRequest::new(vec![Message::human("给我写一首诗")]);
let mut stream = model.stream_chat(&request).await?;

while let Some(chunk) = stream.next().await {
    let chunk = chunk?;
    if let Some(text) = &chunk.content {
        print!("{}", text);
    }
}
```

### Embeddings

```rust,ignore
use std::sync::Arc;
use synaptic::openai::{AzureOpenAiConfig, AzureOpenAiEmbeddings};
use synaptic::models::HttpBackend;
use synaptic::core::Embeddings;

let config = AzureOpenAiConfig::new(
    "your-api-key",
    "my-resource",
    "text-embedding-3-small",  // 部署名称
);

let embeddings = AzureOpenAiEmbeddings::new(config, Arc::new(HttpBackend::new()));
let vectors = embeddings.embed_documents(&["Hello world", "你好世界"]).await?;
```

## 工具调用

Azure OpenAI 支持工具调用，使用方式与标准 OpenAI 模型相同：

```rust,ignore
use synaptic::core::{ChatRequest, Message, ToolDefinition};

let tools = vec![ToolDefinition {
    name: "get_weather".to_string(),
    description: "获取指定城市的天气".to_string(),
    parameters: serde_json::json!({
        "type": "object",
        "properties": {
            "city": { "type": "string", "description": "城市名称" }
        },
        "required": ["city"]
    }),
}];

let request = ChatRequest::new(vec![Message::human("北京天气怎么样？")])
    .with_tools(tools);

let response = model.chat(&request).await?;
```

## 配置参考

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `api_key` | `String` | 必填 | Azure OpenAI API 密钥 |
| `resource_name` | `String` | 必填 | Azure OpenAI 资源名称 |
| `deployment_name` | `String` | 必填 | 模型部署名称 |
| `api_version` | `String` | `"2024-10-21"` | API 版本 |
| `base_url` | `Option<String>` | 自动生成 | 可选的自定义 base URL |
