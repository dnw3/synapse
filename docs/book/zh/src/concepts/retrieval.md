# 检索

检索增强生成（RAG）使 LLM 的回答以外部知识为基础。RAG 系统不仅仅依赖模型在训练期间学到的内容，还会在查询时检索相关文档并将其包含在提示词中。本页解释检索管道的架构、每个组件的作用，以及 Synaptic 提供的检索器类型。

## 管道

一个 RAG 管道包含五个阶段：

```
Load  -->  Split  -->  Embed  -->  Store  -->  Retrieve
```

1. **加载（Load）**：从文件、数据库或网络中将原始内容读入 `Document` 结构体。
2. **分割（Split）**：将大文档拆分为更小的、语义连贯的片段。
3. **嵌入（Embed）**：将文本片段转换为捕捉语义的数值向量。
4. **存储（Store）**：将向量索引化，以便高效进行相似度搜索。
5. **检索（Retrieve）**：给定查询，找到最相关的片段。

每个阶段都有专门的 trait 和多种实现。你可以根据数据源和需求在每个阶段混合搭配不同的实现。

## Document

`Document` 结构体是通用的内容单元：

```rust
pub struct Document {
    pub id: Option<String>,
    pub content: String,
    pub metadata: HashMap<String, Value>,
}
```

- `content` 保存文本内容。
- `metadata` 保存任意的键值对（源文件名、页码、章节标题、创建日期等）。
- `id` 是一个可选的唯一标识符，被存储用于更新和删除操作。

Document 在管道的每个阶段流转。加载器生产它们，分割器转换它们（保留并增强 metadata），检索器返回它们。

## 加载

`Loader` trait 是异步的，返回一个 Document 流：

| 加载器 | 数据源 | 行为 |
|--------|--------|----------|
| `TextLoader` | 纯文本文件 | 每个文件一个 Document |
| `JsonLoader` | JSON 文件 | 可配置的 `id_key` 和 `content_key` 提取 |
| `CsvLoader` | CSV 文件 | 基于列，其他列作为 metadata |
| `DirectoryLoader` | 文件目录 | 递归遍历，支持 glob 过滤文件类型 |
| `FileLoader` | 单个文件 | 通用文件加载，可配置解析器 |
| `MarkdownLoader` | Markdown 文件 | Markdown 感知解析 |
| `WebLoader` | URL | 获取并处理网页内容 |

加载器处理读取和解析的机制。它们生产带有适当 metadata 的 `Document` 值（例如，包含文件路径的 `source` 字段）。

## 分割

大文档必须被分割成适合嵌入模型上下文窗口的片段，并且包含聚焦、连贯的内容。`TextSplitter` trait 提供：

```rust
pub trait TextSplitter: Send + Sync {
    fn split_text(&self, text: &str) -> Result<Vec<String>, SynapticError>;
    fn split_documents(&self, documents: Vec<Document>) -> Result<Vec<Document>, SynapticError>;
}
```

| 分割器 | 策略 |
|----------|----------|
| `CharacterTextSplitter` | 按单个分隔符分割（默认：`"\n\n"`），可配置片段大小和重叠 |
| `RecursiveCharacterTextSplitter` | 尝试分隔符层级（`"\n\n"`、`"\n"`、`" "`、`""`）——在片段大小范围内按最大单元分割 |
| `MarkdownHeaderTextSplitter` | 按 Markdown 标题分割，将标题层级添加到 metadata |
| `HtmlHeaderTextSplitter` | 按 HTML 标题标签分割，将标题层级添加到 metadata |
| `TokenTextSplitter` | 基于近似 token 计数分割（~4 字符/token 启发式，词边界感知） |
| `LanguageTextSplitter` | 使用语言感知分隔符分割代码（函数、类等） |

最常用的分割器是 `RecursiveCharacterTextSplitter`。它生产的片段尊重自然的文档边界（段落、然后句子、然后词），并在片段之间包含可配置的重叠，以避免片段边界处的信息丢失。

`split_documents()` 会在每个片段上保留原始文档的 metadata，因此你可以追溯每个片段到其来源。

## 嵌入

嵌入模型将文本转换为密集的数值向量。语义相似的文本会在向量空间中产生距离相近的向量。对应的 trait：

```rust
#[async_trait]
pub trait Embeddings: Send + Sync {
    async fn embed_documents(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, SynapticError>;
    async fn embed_query(&self, text: &str) -> Result<Vec<f32>, SynapticError>;
}
```

两个方法是因为某些提供商对文档（可能批量处理）和查询（单个文本，可能使用不同的提示词前缀）有不同的优化策略。

| 实现 | 描述 |
|----------------|-------------|
| `OpenAiEmbeddings` | OpenAI 的嵌入 API（text-embedding-ada-002 等） |
| `OllamaEmbeddings` | 本地 Ollama 嵌入模型 |
| `FakeEmbeddings` | 用于测试的确定性向量（无 API 调用） |
| `CachedEmbeddings` | 包装任意 `Embeddings`，添加缓存以避免冗余 API 调用 |

## 向量存储

向量存储保存嵌入后的文档并支持相似度搜索：

```rust
#[async_trait]
pub trait VectorStore: Send + Sync {
    async fn add_documents(&self, docs: Vec<Document>, embeddings: Vec<Vec<f32>>) -> Result<Vec<String>, SynapticError>;
    async fn similarity_search(&self, query_embedding: &[f32], k: usize) -> Result<Vec<Document>, SynapticError>;
    async fn delete(&self, ids: &[String]) -> Result<(), SynapticError>;
}
```

`InMemoryVectorStore` 使用余弦相似度进行暴力搜索。它将文档及其嵌入存储在 `RwLock<HashMap>` 中，在查询时计算与所有存储向量的余弦相似度，并返回 top-k 结果。适用于中小规模集合（数千个文档）。对于更大的集合，你需要使用专用的向量数据库来实现 `VectorStore` trait。

## 检索

`Retriever` trait 是查询时的接口：

```rust
#[async_trait]
pub trait Retriever: Send + Sync {
    async fn retrieve(&self, query: &str) -> Result<Vec<Document>, SynapticError>;
}
```

检索器接受自然语言查询并返回相关文档。Synaptic 提供了七种检索器实现，各有不同的优势。

### InMemoryRetriever

最简单的检索器——将文档存储在内存中，基于关键词匹配返回结果。适用于测试和小型集合。

### BM25Retriever

实现了 Okapi BM25 评分算法，这是一种经典的信息检索方法，根据词频和逆文档频率对文档进行排名。不需要嵌入——纯粹的词汇匹配。

BM25 擅长精确的关键词匹配。如果用户搜索 "tokio runtime"，而某个文档恰好包含这些词，BM25 会给予高排名，即使使用不同措辞的语义相似文档得分较低。

### MultiQueryRetriever

使用 LLM 从原始查询生成多个查询变体，然后通过基础检索器运行每个变体并合并结果。这解决了单一查询措辞可能遗漏相关文档的问题：

```
Original query: "How do I handle errors?"
Generated variants:
  - "What is the error handling approach?"
  - "How are errors propagated in the system?"
  - "What error types are available?"
```

### EnsembleRetriever

使用倒数排名融合（RRF）合并多个检索器的结果。典型的设置是将 BM25（擅长精确匹配）与向量存储检索器（擅长语义匹配）配对：

RRF 算法根据文档在各个检索器中的排名位置分配分数，因此在多个检索器的 top 结果中都出现的文档会获得更高的综合分数。

### ContextualCompressionRetriever

包装基础检索器并压缩检索到的文档，移除不相关的内容。使用 `DocumentCompressor`（例如 `EmbeddingsFilter`，用于过滤掉低于相似度阈值的文档）在检索后优化结果。

### SelfQueryRetriever

使用 LLM 将用户的查询解析为文档 metadata 上的结构化过滤条件，结合语义搜索查询。例如：

```
User query: "Find papers about transformers published after 2020"
Parsed:
  - Semantic query: "papers about transformers"
  - Metadata filter: year > 2020
```

这支持将语义搜索与精确 metadata 过滤相结合的自然语言查询。

### ParentDocumentRetriever

存储小的子片段用于嵌入（提高检索精度），但返回它们所属的更大的父文档（为 LLM 提供更多上下文）。这解决了小片段（有利于匹配）和大片段（有利于上下文）之间的矛盾。

### MultiVectorRetriever

与 `ParentDocumentRetriever` 类似，但在向量存储层实现。`MultiVectorRetriever` 将子文档嵌入存储在 `VectorStore` 中，并维护一个单独的 docstore 将子 ID 映射到父文档。查询时，它搜索匹配的子片段，然后查找其父文档返回。此功能在 `synaptic-vectorstores` 中可用。

## 将检索连接到生成

检索器产出 `Vec<Document>`。要在 RAG 链中使用它们，通常需要将文档格式化到提示词中并传递给 LLM：

```rust
// Pseudocode for a RAG chain
let docs = retriever.retrieve("What is Synaptic?").await?;
let context = docs.iter().map(|d| d.content.as_str()).collect::<Vec<_>>().join("\n\n");
let prompt = format!("Context:\n{context}\n\nQuestion: What is Synaptic?");
```

使用 LCEL，可以将其组合为一个可复用的链，使用 `RunnableParallel`（同时获取上下文和透传问题）、`RunnableLambda`（格式化提示词）和聊天模型。

## 参见

- [文档加载器](../how-to/retrieval/loaders.md) -- 从文件和网络加载数据
- [文本分割器](../how-to/retrieval/splitters.md) -- 将文档分割为片段
- [嵌入模型](../how-to/retrieval/embeddings.md) -- 用于向量搜索的嵌入模型
- [向量存储](../how-to/retrieval/vector-stores.md) -- 存储和搜索向量
- [BM25 检索器](../how-to/retrieval/bm25.md) -- 基于关键词的检索
- [Ensemble 检索器](../how-to/retrieval/ensemble.md) -- 组合多个检索器
- [Self-Query 检索器](../how-to/retrieval/self-query.md) -- LLM 驱动的 metadata 过滤
- [Runnables 与 LCEL](runnables-lcel.md) -- 将检索组合到链中
