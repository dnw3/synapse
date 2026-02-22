# Jina Reader

[Jina Reader](https://jina.ai/reader/) 将任意网页 URL 转换为适合 LLM 消费的干净 Markdown 内容。它会自动去除广告、导航菜单和冗余内容，只保留正文。无需 API 密钥。

## 安装

```toml
[dependencies]
synaptic = { version = "0.2", features = ["tools"] }
```

无需 API 密钥。

## 使用示例

```rust,ignore
use synaptic::tools::JinaReaderTool;
use synaptic::core::Tool;
use serde_json::json;

let tool = JinaReaderTool::new();

let result = tool.call(json!({
    "url": "https://blog.rust-lang.org/2025/01/01/Rust-1.84.0.html"
})).await?;

println!("{}", result["content"].as_str().unwrap());
```

## 配合 Agent 使用

```rust,ignore
use synaptic::tools::{JinaReaderTool, ToolRegistry};
use std::sync::Arc;

let registry = ToolRegistry::new();
registry.register(Arc::new(JinaReaderTool::new()))?;
```

## 注意事项

- Jina Reader 免费使用，轻度使用无需认证。
- 返回内容为 Markdown 格式，便于直接嵌入 LLM 提示词。
- 高频使用时，建议申请 Jina AI API 密钥以获得更高限速。
- 工具会在请求中添加 `X-Return-Format: markdown` 头以获取 Markdown 格式输出。
