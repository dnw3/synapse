# Brave Search

[Brave Search](https://search.brave.com/) 提供注重隐私的网络搜索服务，拥有独立索引。`BraveSearchTool` 将 Brave Web Search API 集成到 Synaptic 智能体中。

## 安装

```toml
[dependencies]
synaptic = { version = "0.2", features = ["tools"] }
```

在 [brave.com/search/api](https://brave.com/search/api/) 获取 API 密钥。

## 使用示例

```rust,ignore
use synaptic::tools::BraveSearchTool;
use synaptic::core::Tool;
use serde_json::json;

let tool = BraveSearchTool::new("your-api-key")
    .with_max_results(5);

let result = tool.call(json!({"query": "Rust 异步运行时对比"})).await?;
println!("{}", serde_json::to_string_pretty(&result)?);
```

## 配合 Agent 使用

```rust,ignore
use synaptic::tools::{BraveSearchTool, ToolRegistry};
use std::sync::Arc;

let registry = ToolRegistry::new();
registry.register(Arc::new(BraveSearchTool::new("your-api-key")))?;
```

## 配置选项

| 选项 | 默认值 | 说明 |
|---|---|---|
| `with_max_results(n)` | `5` | 返回的最大搜索结果数量 |

## 注意事项

- 每条结果包含标题、URL 和描述。
- Brave Search API 有免费和付费版本，请参阅 [brave.com/search/api](https://brave.com/search/api/) 了解限速规则。
- Brave Search 维护独立索引，适合注重隐私的部署场景。
