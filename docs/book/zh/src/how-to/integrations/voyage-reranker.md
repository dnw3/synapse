# Voyage AI 重排序器

Voyage AI 的重排序模型是高质量的交叉编码器，可显著提升检索精度。由 Voyage 嵌入模型团队出品，专为 RAG 应用优化。

## 配置

```toml
[dependencies]
synaptic = { version = "0.2", features = ["voyage"] }
```

在 [voyageai.com](https://www.voyageai.com/) 注册并创建 API 密钥。

## 可用模型

| 变体 | API 模型 ID | 最佳场景 |
|------|------------|---------|
| `Rerank2` | `rerank-2` | 通用（推荐） |
| `Rerank2Lite` | `rerank-2-lite` | 快速、低成本 |
| `Custom(String)` | _（任意）_ | 未列出的模型 |

## 使用方法

```rust,ignore
use synaptic::voyage::reranker::{VoyageReranker, VoyageRerankerModel};
use synaptic::core::Document;

let reranker = VoyageReranker::new("pa-your-api-key")
    .with_model(VoyageRerankerModel::Rerank2);

let docs = vec![
    Document::new("doc1", "巴黎是法国的首都。"),
    Document::new("doc2", "埃菲尔铁塔位于巴黎。"),
    Document::new("doc3", "柏林是德国的首都。"),
    Document::new("doc4", "法国是西欧的一个国家。"),
];

let results = reranker
    .rerank("法国的首都是哪里？", docs, 2)
    .await?;

for (doc, score) in &results {
    println!("{:.4}: {}", score, doc.content);
}
// 输出示例：
// 0.9234: 巴黎是法国的首都。
// 0.6821: 法国是西欧的一个国家。
```

## RAG 流水线集成

```rust,ignore
use synaptic::voyage::reranker::VoyageReranker;
use synaptic::voyage::VoyageEmbeddings;
use synaptic::vectorstores::InMemoryVectorStore;

// 通过快速向量搜索检索 20 个候选文档
let candidates = vector_store
    .similarity_search("法国首都", 20, &embeddings)
    .await?;

// 使用 Voyage 交叉编码器重排序到前 5 位
let reranker = VoyageReranker::new("pa-your-api-key");
let top5 = reranker
    .rerank("法国首都", candidates, 5)
    .await?;

// 将 top5 作为 LLM 的上下文
```

## 自定义端点

指向自定义或自托管部署：

```rust,ignore
let reranker = VoyageReranker::new("pa-key")
    .with_base_url("https://custom.voyageai.com/v1");
```

## 错误处理

```rust,ignore
use synaptic::core::SynapticError;

match reranker.rerank(query, docs, k).await {
    Ok(results) => {
        for (doc, score) in results {
            println!("{:.4}: {}", score, doc.content);
        }
    }
    Err(SynapticError::Retriever(msg)) => eprintln!("重排序错误: {}", msg),
    Err(e) => return Err(e.into()),
}
```

## 配置参考

| 参数 | 默认值 | 描述 |
|------|--------|------|
| `api_key` | 必填 | Voyage AI API 密钥（`pa-...`） |
| `model` | `Rerank2` | 重排序器模型 |
| `base_url` | Voyage AI URL | 自定义部署时覆盖此项 |
