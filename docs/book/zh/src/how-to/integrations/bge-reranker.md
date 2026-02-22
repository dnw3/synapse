# BGE 重排序器（HuggingFace）

BAAI 的 BGE 重排序器模型是最先进的交叉编码器重排序器，可通过 HuggingFace Inference API 使用。与双编码器嵌入相似度相比，它们在文档排序方面具有显著优势，是 RAG 流水线最终重排序阶段的理想选择。

## 配置

```toml
[dependencies]
synaptic = { version = "0.2", features = ["huggingface"] }
```

在 [huggingface.co](https://huggingface.co/) 注册并在「设置 → 访问令牌」中创建访问令牌。

## 可用模型

| 变体 | HF 模型 ID | 上下文 | 最佳场景 |
|------|------------|--------|---------|
| `BgeRerankerV2M3` | `BAAI/bge-reranker-v2-m3` | 512 tokens | 多语言（推荐） |
| `BgeRerankerLarge` | `BAAI/bge-reranker-large` | 512 tokens | 最高质量（英语） |
| `BgeRerankerBase` | `BAAI/bge-reranker-base` | 512 tokens | 快速高质量（英语） |
| `Custom(String)` | _（任意）_ | — | 未列出的模型 |

## 使用方法

```rust,ignore
use synaptic::huggingface::reranker::{BgeRerankerModel, HuggingFaceReranker};
use synaptic::core::Document;

let reranker = HuggingFaceReranker::new("hf_your_access_token")
    .with_model(BgeRerankerModel::BgeRerankerV2M3);

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
// 0.9876: 巴黎是法国的首都。
// 0.7543: 法国是西欧的一个国家。
```

## RAG 流水线集成

使用 BGE 重排序器将大量候选文档重新排序为小规模高精度集合，提升检索质量：

```rust,ignore
use synaptic::huggingface::reranker::HuggingFaceReranker;
use synaptic::vectorstores::InMemoryVectorStore;
use synaptic::core::Document;

// 通过快速向量搜索检索 20 个候选文档
let candidates = vector_store
    .similarity_search("法国首都", 20, &embeddings)
    .await?;

// 使用交叉编码器重排序到前 5 位
let reranker = HuggingFaceReranker::new("hf_token");
let top5 = reranker
    .rerank("法国首都", candidates, 5)
    .await?;

// 将 top5 作为 LLM 的上下文
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
| `api_key` | 必填 | HuggingFace 访问令牌（`hf_...`） |
| `model` | `BgeRerankerV2M3` | 重排序器模型 |
| `base_url` | HF inference URL | 自定义部署时覆盖此项 |
