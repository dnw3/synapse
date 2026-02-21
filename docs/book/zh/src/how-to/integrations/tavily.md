# Tavily 搜索工具

本指南展示如何使用 Synaptic 的 Tavily 集成为 Agent 添加网络搜索能力。[Tavily](https://tavily.com/) 提供专为 AI Agent 和 RAG 应用优化的搜索 API。

## 设置

在 `Cargo.toml` 中添加 `tavily` feature：

```toml
[dependencies]
synaptic = { version = "0.2", features = ["openai", "tavily"] }
```

设置 API 密钥环境变量：

```bash
export TAVILY_API_KEY="tvly-..."
```

## 配置

使用 `TavilyConfig` 创建配置：

```rust,ignore
use synaptic::tavily::{TavilyConfig, TavilySearchTool};

let config = TavilyConfig::new("tvly-...");
let tool = TavilySearchTool::new(config);
```

### 自定义搜索参数

```rust,ignore
let config = TavilyConfig::new("tvly-...")
    .with_max_results(5)
    .with_search_depth("advanced");

let tool = TavilySearchTool::new(config);
```

### 搜索深度

- `"basic"` -- 默认，快速搜索
- `"advanced"` -- 更深入的搜索，返回更详细的结果（耗时稍长）

## 用法

### 作为 Tool 使用

`TavilySearchTool` 实现了 `Tool` trait，工具名称为 `"tavily_search"`，参数格式为 `{"query": "搜索内容"}`：

```rust,ignore
use synaptic::tavily::{TavilyConfig, TavilySearchTool};
use synaptic::core::Tool;

let tool = TavilySearchTool::new(TavilyConfig::new("tvly-..."));

// 直接调用
let result = tool.call(r#"{"query": "Rust 编程语言最新动态"}"#).await?;
println!("{}", result);
```

### 注册到 ToolRegistry

```rust,ignore
use std::sync::Arc;
use synaptic::tools::ToolRegistry;
use synaptic::tavily::{TavilyConfig, TavilySearchTool};

let mut registry = ToolRegistry::new();
registry.register(Arc::new(
    TavilySearchTool::new(TavilyConfig::new("tvly-...")),
));
```

### 与 ReAct Agent 配合使用

将 Tavily 搜索工具添加到 ReAct Agent 中，使 Agent 能够搜索网络获取实时信息：

```rust,ignore
use std::sync::Arc;
use synaptic::core::{ChatModel, Tool};
use synaptic::openai::OpenAiChatModel;
use synaptic::tavily::{TavilyConfig, TavilySearchTool};
use synaptic::graph::create_react_agent;

let model: Arc<dyn ChatModel> = Arc::new(OpenAiChatModel::new("gpt-4o"));

let tools: Vec<Arc<dyn Tool>> = vec![
    Arc::new(TavilySearchTool::new(
        TavilyConfig::new("tvly-...")
            .with_max_results(3),
    )),
];

let agent = create_react_agent(model, tools, Default::default())?;
```

### 工具参数 Schema

`TavilySearchTool` 生成的 `ToolDefinition` 参数 schema 如下：

```json
{
    "type": "object",
    "properties": {
        "query": {
            "type": "string",
            "description": "The search query"
        }
    },
    "required": ["query"]
}
```

## 配置参考

| 字段 | 类型 | 默认值 | 说明 |
|------|------|--------|------|
| `api_key` | `String` | 必填 | Tavily API 密钥 |
| `max_results` | `usize` | `5` | 最大返回结果数 |
| `search_depth` | `String` | `"basic"` | 搜索深度（`"basic"` 或 `"advanced"`） |
| `base_url` | `String` | `"https://api.tavily.com"` | API Base URL |
