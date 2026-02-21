# AWS Bedrock

本指南展示如何使用 Synaptic 接入 [AWS Bedrock](https://aws.amazon.com/bedrock/)。Bedrock 提供多种基础模型（Claude、Llama、Mistral 等）的托管访问，通过 AWS SDK 进行认证。

## 设置

在 `Cargo.toml` 中添加 `bedrock` feature：

```toml
[dependencies]
synaptic = { version = "0.2", features = ["bedrock"] }
```

Bedrock 使用 AWS SDK 进行认证，通过标准 AWS 环境变量配置凭证：

```bash
export AWS_ACCESS_KEY_ID="AKIA..."
export AWS_SECRET_ACCESS_KEY="..."
export AWS_REGION="us-east-1"  # 可选，默认为 us-east-1
```

也支持 AWS 配置文件（`~/.aws/credentials`）和 IAM 角色等标准 AWS 凭证方式。

> **注意：** Bedrock 不使用 `ProviderBackend`（`HttpBackend`/`FakeBackend`），而是直接使用 AWS SDK 的 `BedrockRuntimeClient`。

## 配置

使用 `BedrockConfig` 创建配置：

```rust,ignore
use synaptic::bedrock::{BedrockConfig, BedrockChatModel};

let config = BedrockConfig::new("anthropic.claude-3-5-sonnet-20241022-v2:0");
```

### 指定 AWS Region

```rust,ignore
let config = BedrockConfig::new("anthropic.claude-3-5-sonnet-20241022-v2:0")
    .with_region("eu-west-1");
```

### 推理参数

```rust,ignore
let config = BedrockConfig::new("anthropic.claude-3-5-sonnet-20241022-v2:0")
    .with_max_tokens(4096)
    .with_temperature(0.7);
```

## 用法

### 创建模型

构造器是**异步**的，因为它需要初始化 AWS SDK 客户端：

```rust,ignore
use synaptic::bedrock::{BedrockConfig, BedrockChatModel};
use synaptic::core::{ChatModel, ChatRequest, Message};

let config = BedrockConfig::new("anthropic.claude-3-5-sonnet-20241022-v2:0");
let model = BedrockChatModel::new(config).await;

let request = ChatRequest::new(vec![Message::human("你好！")]);
let response = model.chat(&request).await?;
println!("{}", response.message.content().unwrap_or_default());
```

### 使用已有的 AWS 客户端

如果你的应用已经有配置好的 `BedrockRuntimeClient`，可以直接传入：

```rust,ignore
use aws_sdk_bedrockruntime::Client;
use synaptic::bedrock::{BedrockConfig, BedrockChatModel};

// 假设你已有一个配置好的 AWS 客户端
let aws_config = aws_config::load_defaults(aws_config::BehaviorVersion::latest()).await;
let client = Client::new(&aws_config);

let config = BedrockConfig::new("anthropic.claude-3-5-sonnet-20241022-v2:0");
let model = BedrockChatModel::from_client(config, client);
```

### 流式输出

Bedrock 支持流式输出：

```rust,ignore
use futures::StreamExt;
use synaptic::core::{ChatModel, ChatRequest, Message};

let request = ChatRequest::new(vec![Message::human("写一个简短的故事")]);
let mut stream = model.stream_chat(&request).await?;

while let Some(chunk) = stream.next().await {
    let chunk = chunk?;
    if let Some(text) = &chunk.content {
        print!("{}", text);
    }
}
```

### 工具调用

Bedrock 支持工具调用（支持 Anthropic Claude 和其他兼容模型）：

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

let request = ChatRequest::new(vec![Message::human("上海天气怎么样？")])
    .with_tools(tools);

let response = model.chat(&request).await?;
```

## 常用模型 ID

| 模型 | Model ID |
|------|----------|
| Claude 3.5 Sonnet v2 | `anthropic.claude-3-5-sonnet-20241022-v2:0` |
| Claude 3.5 Haiku | `anthropic.claude-3-5-haiku-20241022-v1:0` |
| Claude 3 Opus | `anthropic.claude-3-opus-20240229-v1:0` |
| Llama 3.1 70B | `meta.llama3-1-70b-instruct-v1:0` |
| Mistral Large | `mistral.mistral-large-2407-v1:0` |

> 完整的模型 ID 列表请参考 [AWS Bedrock 文档](https://docs.aws.amazon.com/bedrock/latest/userguide/models-supported.html)。

## 配置参考

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `model_id` | `String` | 必填 | Bedrock 模型标识符 |
| `region` | `Option<String>` | 从环境变量读取 | AWS Region |
| `max_tokens` | `Option<u32>` | `None`（使用模型默认值） | 最大生成 token 数 |
| `temperature` | `Option<f32>` | `None`（使用模型默认值） | 采样温度 |
