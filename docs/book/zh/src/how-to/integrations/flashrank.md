# FlashRank（本地重排序器）

FlashRank 是基于 BM25 评分的快速零依赖本地重排序器。它完全在进程内运行，无需任何外部 API 调用，非常适合开发、测试和离线场景。

## 配置

```toml
[dependencies]
synaptic = { version = "0.2", features = ["flashrank"] }
```

无需 API 密钥，无需外部服务。

## 工作原理

FlashRank 使用 Okapi BM25 算法（与 Elasticsearch 默认排名相同的基础）对文档进行评分。它对查询和文档进行分词处理，计算带长度归一化的词频，并按相关性评分降序返回结果。

**优点：**
- 零延迟（无网络调用）
- 无 API 费用
- 支持离线和 CI/CD 环境
- 完全确定性

**缺点：**
- 仅支持词法匹配（无语义理解）
- 除词元重叠外无多语言支持
- 对于复杂查询，精度低于神经网络重排序器

对于需要语义理解的生产场景，请考虑使用 [BGE 重排序器](bge-reranker.md)、[Voyage AI 重排序器](voyage-reranker.md) 或 [Jina AI 重排序器](jina.md)。

## 使用方法

```rust,ignore
use synaptic::flashrank::{FlashRankConfig, FlashRankReranker};
use synaptic::core::Document;

let reranker = FlashRankReranker::new(FlashRankConfig::default());

let docs = vec![
    Document::new("doc1", "巴黎是法国的首都，埃菲尔铁塔的所在地。"),
    Document::new("doc2", "柏林是德国的首都。"),
    Document::new("doc3", "今天天气晴朗。"),
    Document::new("doc4", "法国是西欧的一个国家。"),
];

let results = reranker
    .rerank("法国首都", docs, 2)
    .await?;

for (doc, score) in &results {
    println!("{:.4}: {}", score, doc.content);
}
// 输出：
// 0.6543: 巴黎是法国的首都，埃菲尔铁塔的所在地。
// 0.2341: 法国是西欧的一个国家。
```

## 参数配置

```rust,ignore
use synaptic::flashrank::FlashRankConfig;

// 使用默认值（k1=1.5, b=0.75 — 标准 BM25 参数）
let config = FlashRankConfig::default();

// 调整 BM25 参数
let config = FlashRankConfig::default()
    .with_k1(1.2)   // 词频饱和度（越低对词频变化越不敏感）
    .with_b(0.8);   // 长度归一化（1.0=全归一化, 0.0=不归一化）
```

## RAG 流水线集成

FlashRank 非常适合作为轻量级初步重排序器或用于开发/测试：

```rust,ignore
use synaptic::flashrank::{FlashRankConfig, FlashRankReranker};
use synaptic::vectorstores::InMemoryVectorStore;

// 通过向量搜索检索 20 个候选文档
let candidates = vector_store
    .similarity_search("法国首都", 20, &embeddings)
    .await?;

// 使用 BM25 本地重排序
let reranker = FlashRankReranker::new(FlashRankConfig::default());
let top5 = reranker
    .rerank("法国首都", candidates, 5)
    .await?;
```

## 升级到神经网络重排序

当需要更高精度时，FlashRank 与神经网络重排序器共享相同的 API 形式，迁移非常简单：

```rust,ignore
// 开发环境：本地 BM25 重排序器
let reranker = synaptic::flashrank::FlashRankReranker::new(Default::default());

// 生产环境：通过 HuggingFace 使用神经交叉编码器
let reranker = synaptic::huggingface::reranker::HuggingFaceReranker::new("hf_token");

// 两者调用方式完全相同：
let results = reranker.rerank(query, docs, top_k).await?;
```

## 配置参考

| 参数 | 默认值 | 描述 |
|------|--------|------|
| `k1` | `1.5` | BM25 词频饱和度。大多数场景推荐范围：1.2–2.0 |
| `b` | `0.75` | BM25 长度归一化。范围：0.0（不归一化）到 1.0（完全归一化） |
