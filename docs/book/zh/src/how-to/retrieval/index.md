# 检索

Synaptic 提供了完整的检索增强生成（RAG）流水线。该流水线分为五个阶段：

1. **加载** -- 从文件、JSON、CSV、网页 URL 或整个目录中摄取原始数据。
2. **分割** -- 将大型文档拆分为适合上下文窗口的较小块。
3. **嵌入** -- 使用 Embeddings 模型将文本块转换为数值向量。
4. **存储** -- 将 Embeddings 持久化到 VectorStore 中，以便进行高效的相似性搜索。
5. **检索** -- 根据给定查询找到最相关的文档。

## 关键类型

| 类型 | Crate | 用途 |
|------|-------|---------|
| `Document` | `synaptic_retrieval` | 包含 `id`、`content` 和 `metadata: HashMap<String, Value>` 的文本单元 |
| `Loader` trait | `synaptic_loaders` | 用于从各种来源加载文档的异步 trait |
| `TextSplitter` trait | `synaptic_splitters` | 将文本分割为块，支持可选的重叠 |
| `Embeddings` trait | `synaptic_embeddings` | 将文本转换为向量表示 |
| `VectorStore` trait | `synaptic_vectorstores` | 存储和搜索文档 Embeddings |
| `Retriever` trait | `synaptic_retrieval` | 根据查询字符串检索相关文档 |

## Retriever 列表

Synaptic 内置了七种 Retriever 实现，各自适用于不同的使用场景：

| Retriever | 策略 |
|-----------|----------|
| `VectorStoreRetriever` | 封装任意 `VectorStore`，进行 cosine similarity 搜索 |
| `BM25Retriever` | Okapi BM25 关键词评分 -- 无需 Embeddings |
| `MultiQueryRetriever` | 使用 LLM 生成查询变体，分别检索后去重 |
| `EnsembleRetriever` | 通过 Reciprocal Rank Fusion 组合多个 Retriever |
| `ContextualCompressionRetriever` | 使用 `DocumentCompressor` 对检索结果进行后过滤 |
| `SelfQueryRetriever` | 使用 LLM 从自然语言中提取结构化元数据过滤条件 |
| `ParentDocumentRetriever` | 搜索小的子块，但返回完整的父文档 |

## 指南

- [文档加载器](loaders.md) -- 从文本、JSON、CSV、文件、目录和网页加载数据
- [文本分割器](splitters.md) -- 使用字符、递归、Markdown 或 Token 策略将文档拆分为块
- [Embeddings](embeddings.md) -- 使用 OpenAI、Ollama 或确定性伪 Embeddings 对文本进行嵌入
- [Vector Stores](vector-stores.md) -- 使用 `InMemoryVectorStore` 存储和搜索 Embeddings
- [BM25 Retriever](bm25.md) -- 基于 Okapi BM25 评分的关键词检索
- [Multi-Query Retriever](multi-query.md) -- 通过生成多个查询视角提高召回率
- [Ensemble Retriever](ensemble.md) -- 使用 Reciprocal Rank Fusion 组合多个 Retriever
- [Contextual Compression](compression.md) -- 使用 Embeddings 相似度阈值对结果进行后过滤
- [Self-Query Retriever](self-query.md) -- 基于 LLM 的自然语言元数据过滤
- [Parent Document Retriever](parent-document.md) -- 搜索小块，返回完整的父文档
