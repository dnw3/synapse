# 自查询检索器

本指南介绍如何使用 `SelfQueryRetriever`，从自然语言查询中自动提取结构化的元数据过滤条件。

## 概述

用户经常在同一句话中表达包含语义查询和元数据约束的搜索意图。例如：

> "查找 2024 年之后发布的关于 Rust 的文档"

这包含：
- 一个 **语义查询**："关于 Rust 的文档"
- 一个 **元数据过滤条件**：`year > 2024`

`SelfQueryRetriever` 使用 `ChatModel` 将用户的自然语言查询解析为结构化的搜索查询加元数据过滤条件，然后将这些过滤条件应用于基础检索器的结果。

## 定义元数据字段

首先，使用 `MetadataFieldInfo` 描述文档集合中可用的元数据字段：

```rust
use synaptic_retrieval::MetadataFieldInfo;

let fields = vec![
    MetadataFieldInfo {
        name: "year".to_string(),
        description: "The year the document was published".to_string(),
        field_type: "integer".to_string(),
    },
    MetadataFieldInfo {
        name: "language".to_string(),
        description: "The programming language discussed".to_string(),
        field_type: "string".to_string(),
    },
    MetadataFieldInfo {
        name: "author".to_string(),
        description: "The author of the document".to_string(),
        field_type: "string".to_string(),
    },
];
```

每个字段包含 `name`（名称）、人类可读的 `description`（描述），以及告知 LLM 预期值类型的 `field_type`（字段类型）。

## 基本用法

```rust
use std::sync::Arc;
use synaptic_retrieval::{SelfQueryRetriever, MetadataFieldInfo, Retriever};

let base_retriever: Arc<dyn Retriever> = Arc::new(/* any retriever */);
let model: Arc<dyn ChatModel> = Arc::new(/* any ChatModel */);

let retriever = SelfQueryRetriever::new(base_retriever, model, fields);

let results = retriever.retrieve(
    "find articles about Rust written by Alice",
    5,
).await?;
// LLM 提取：query="Rust", filters: [language eq "Rust", author eq "Alice"]
```

## 工作原理

1. 检索器构建一个描述可用元数据字段的提示，并将用户的查询发送给 LLM。
2. LLM 返回一个 JSON 对象，包含：
   - `"query"` -- 提取的语义搜索查询。
   - `"filters"` -- 过滤条件对象数组，每个对象包含 `"field"`、`"op"` 和 `"value"`。
3. 检索器使用提取的查询通过基础检索器进行搜索（获取额外候选项，`top_k * 2`）。
4. 将过滤条件应用于结果，只保留元数据匹配所有过滤条件的文档。
5. 最终过滤后的结果截断为 `top_k` 并返回。

## 支持的过滤运算符

| 运算符 | 含义 |
|--------|------|
| `eq` | 等于 |
| `gt` | 大于 |
| `gte` | 大于或等于 |
| `lt` | 小于 |
| `lte` | 小于或等于 |
| `contains` | 字符串包含子串 |

数值比较支持整数和浮点数。字符串比较使用字典序。

## 完整示例

```rust
use std::sync::Arc;
use std::collections::HashMap;
use synaptic_retrieval::{
    BM25Retriever,
    SelfQueryRetriever,
    MetadataFieldInfo,
    Document,
    Retriever,
};
use serde_json::json;

// 带元数据的文档
let docs = vec![
    Document::with_metadata(
        "1",
        "An introduction to Rust's ownership model",
        HashMap::from([
            ("year".to_string(), json!(2024)),
            ("language".to_string(), json!("Rust")),
        ]),
    ),
    Document::with_metadata(
        "2",
        "Advanced Python patterns for data pipelines",
        HashMap::from([
            ("year".to_string(), json!(2023)),
            ("language".to_string(), json!("Python")),
        ]),
    ),
    Document::with_metadata(
        "3",
        "Rust async programming with Tokio",
        HashMap::from([
            ("year".to_string(), json!(2025)),
            ("language".to_string(), json!("Rust")),
        ]),
    ),
];

let base = Arc::new(BM25Retriever::new(docs));
let model: Arc<dyn ChatModel> = Arc::new(/* your model */);

let fields = vec![
    MetadataFieldInfo {
        name: "year".to_string(),
        description: "Publication year".to_string(),
        field_type: "integer".to_string(),
    },
    MetadataFieldInfo {
        name: "language".to_string(),
        description: "Programming language topic".to_string(),
        field_type: "string".to_string(),
    },
];

let retriever = SelfQueryRetriever::new(base, model, fields);

// 包含隐式过滤条件的自然语言查询
let results = retriever.retrieve("Rust articles from 2025", 5).await?;
// LLM 提取：query="Rust articles", filters: [language eq "Rust", year eq 2025]
// 仅返回文档 3
```

## 注意事项

- 过滤条件提取的质量取决于 LLM。使用能力较强的模型以获得可靠的结果。
- 只有引用 `MetadataFieldInfo` 中声明的字段的过滤条件才会被应用；未知字段会被忽略。
- 如果 LLM 无法将查询解析为结构化过滤条件，则回退到空过滤条件列表，返回标准检索结果。
