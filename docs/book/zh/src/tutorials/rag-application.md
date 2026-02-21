# 构建 RAG 应用

本教程将引导你构建一个 RAG（Retrieval-Augmented Generation，检索增强生成）应用。RAG 让 AI 能够基于你提供的文档内容回答问题，而不仅仅依赖模型的训练数据。

## 你将学到什么

- 使用文档加载器加载原始文档
- 使用文本分割器将长文档拆分为小块
- 使用嵌入模型和向量存储建立索引
- 使用 Retriever 检索相关文档
- 将检索结果与 LLM 结合生成回答

## RAG 管道概述

RAG 管道由五个阶段组成，每个阶段对应 Synaptic 中的一个或多个 crate：

```text
加载文档 --> 拆分文档 --> 嵌入和存储 --> 检索 --> 生成
(loaders)   (splitters)  (embeddings   (retrieval) (models +
                          + vectorstores)            prompts)
```

1. **加载文档** -- 使用 `TextLoader`、`JsonLoader`、`CsvLoader` 或 `DirectoryLoader` 从各种来源加载原始文档
2. **拆分文档** -- 使用 `RecursiveCharacterTextSplitter` 将长文档按层级分隔符拆分为小块
3. **嵌入和存储** -- 使用嵌入模型将文本转为向量，存储到向量数据库中
4. **检索** -- 根据用户查询，从向量数据库中检索最相关的文档块
5. **生成** -- 将检索到的文档块作为上下文，与用户问题一起发送给 LLM

## 前置条件

在 `Cargo.toml` 中添加所需的依赖：

```toml
[dependencies]
synaptic = { version = "0.2", features = ["rag"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## 第一步：加载文档

Synaptic 提供多种文档加载器。最简单的是 `TextLoader`，它从文本文件加载内容并创建 `Document` 对象：

```rust
use synaptic::loaders::{Loader, TextLoader};
use synaptic::retrieval::Document;

// 从文件加载
let loader = TextLoader::new("docs/my-knowledge-base.txt");
let documents = loader.load().await?;

// 也可以直接创建 Document
let documents = vec![
    Document::new("Rust 是一门系统编程语言，注重安全、并发和性能。"),
    Document::new("Synaptic 是一个基于 Rust 的 AI Agent 框架，兼容 LangChain 架构。"),
    Document::new("RAG 通过检索外部知识来增强 LLM 的回答能力。"),
];
```

`Document` 包含三个字段：
- `id` -- 唯一标识符（可选，自动生成）
- `content` -- 文档内容
- `metadata` -- 元数据（`HashMap<String, Value>`），可存储来源、日期等信息

对于更复杂的数据源，可以使用其他加载器：
- **`JsonLoader`** -- 从 JSON 文件加载，可配置 id 和 content 的键名
- **`CsvLoader`** -- 从 CSV 文件加载，基于列映射
- **`DirectoryLoader`** -- 递归加载目录中的文件，支持 glob 过滤

所有加载器都实现了 `Loader` trait，提供 `load()` 方法（一次性加载）和 `lazy_load()` 方法（流式延迟加载）。

## 第二步：拆分文档

长文档需要拆分为较小的块，以便于嵌入和检索。`RecursiveCharacterTextSplitter` 是最常用的分割器，它按层级分隔符（`\n\n` -> `\n` -> ` ` -> `""`）递归拆分文本：

```rust
use synaptic::splitters::{TextSplitter, RecursiveCharacterTextSplitter};

let splitter = RecursiveCharacterTextSplitter::new(500)  // 每块最多 500 个字符
    .with_chunk_overlap(50);  // 相邻块之间重叠 50 个字符

let chunks = splitter.split_documents(&documents);
println!("原始文档: {} 个, 拆分后: {} 个块", documents.len(), chunks.len());
```

`TextSplitter` trait 提供两个方法：
- **`split_text(text)`** -- 拆分单个字符串，返回 `Vec<String>`
- **`split_documents(docs)`** -- 拆分文档列表，返回 `Vec<Document>`，保留原始元数据

其他可用的分割器：
- **`CharacterTextSplitter`** -- 按单一分隔符拆分
- **`MarkdownHeaderTextSplitter`** -- 按 Markdown 标题拆分，标题作为元数据
- **`TokenTextSplitter`** -- 按 token 数量拆分（约 4 字符/token 的启发式估算）

## 第三步：嵌入和存储

将文档块转换为向量并存储到向量数据库中。这里我们使用 `FakeEmbeddings`（确定性测试用嵌入模型）和 `InMemoryVectorStore`：

```rust
use synaptic::embeddings::FakeEmbeddings;
use synaptic::vectorstores::{VectorStore, InMemoryVectorStore};

// 创建嵌入模型（生产环境使用 OpenAiEmbeddings 或 OllamaEmbeddings）
let embeddings = FakeEmbeddings::new();

// 创建向量存储
let store = InMemoryVectorStore::new();

// 添加文档到向量存储（自动计算嵌入向量）
let ids = store.add_documents(chunks, &embeddings).await?;
println!("已索引 {} 个文档块", ids.len());
```

`VectorStore` trait 提供以下方法：
- **`add_documents(docs, embeddings)`** -- 添加文档并计算嵌入
- **`similarity_search(query, k, embeddings)`** -- 按查询文本检索最相似的 k 个文档
- **`similarity_search_with_score(query, k, embeddings)`** -- 检索并返回相似度分数
- **`delete(ids)`** -- 按 ID 删除文档

在生产环境中，你会使用真实的嵌入模型：

```rust
use synaptic::openai::OpenAiEmbeddings;

let embeddings = OpenAiEmbeddings::new("text-embedding-3-small");
```

## 第四步：检索

使用 `VectorStoreRetriever` 将向量存储桥接到 `Retriever` trait，方便与 Synaptic 的其他组件集成：

```rust
use synaptic::vectorstores::VectorStoreRetriever;
use synaptic::retrieval::Retriever;

// 创建检索器，每次检索返回最相似的 3 个文档
let retriever = VectorStoreRetriever::new(store, embeddings, 3);

// 检索相关文档
let relevant_docs = retriever.retrieve("什么是 Synaptic？").await?;
for doc in &relevant_docs {
    println!("检索到: {}", doc.content);
}
```

### 高级检索器

Synaptic 提供多种高级检索策略，可以显著提高检索质量：

- **`BM25Retriever`** -- 基于 Okapi BM25 评分的关键词检索，不依赖嵌入模型
- **`MultiQueryRetriever`** -- 使用 LLM 生成多个查询变体，合并检索结果以提高召回率
- **`EnsembleRetriever`** -- 融合多个检索器的结果（使用 Reciprocal Rank Fusion 算法）
- **`ContextualCompressionRetriever`** -- 对检索结果进行压缩过滤，通过 `DocumentCompressor` trait 和 `EmbeddingsFilter`（基于相似度阈值）移除无关内容
- **`SelfQueryRetriever`** -- LLM 驱动的元数据过滤检索，自动从自然语言查询中提取过滤条件
- **`ParentDocumentRetriever`** -- 子文档到父文档的映射检索，通过小块检索定位，返回完整的大块上下文

## 第五步：生成

将检索到的文档作为上下文，结合用户问题构建提示，发送给 LLM 生成回答：

```rust
use synaptic::core::{ChatModel, ChatRequest, Message, SynapticError};

// 将检索到的文档拼接为上下文
let context = relevant_docs.iter()
    .map(|doc| doc.content.as_str())
    .collect::<Vec<_>>()
    .join("\n\n");

// 构建包含上下文的提示
let messages = vec![
    Message::system(&format!(
        "你是一个有帮助的助手。根据以下上下文回答用户的问题。\
         如果上下文中没有相关信息，请诚实地说你不知道。\n\n\
         上下文：\n{}",
        context
    )),
    Message::human("什么是 Synaptic？"),
];

let request = ChatRequest::new(messages);
let response = model.chat(request).await?;
println!("回答: {}", response.message.content());
```

## 完整示例

以下是将所有步骤组合在一起的完整示例：

```rust
use synaptic::core::{ChatModel, ChatRequest, Message, SynapticError};
use synaptic::embeddings::FakeEmbeddings;
use synaptic::retrieval::{Document, Retriever};
use synaptic::splitters::{TextSplitter, RecursiveCharacterTextSplitter};
use synaptic::vectorstores::{VectorStore, InMemoryVectorStore, VectorStoreRetriever};

#[tokio::main]
async fn main() -> Result<(), SynapticError> {
    // 1. 准备文档
    let documents = vec![
        Document::new("Rust 是一门系统编程语言，注重安全、并发和性能。它通过所有权系统在编译时保证内存安全。"),
        Document::new("Synaptic 是一个基于 Rust 的 AI Agent 框架，兼容 LangChain 架构。它提供了 Chat Models、Tools、Memory、Graph 等组件。"),
        Document::new("RAG（检索增强生成）通过检索外部知识库来增强 LLM 的回答能力，减少幻觉。"),
    ];

    // 2. 拆分文档
    let splitter = RecursiveCharacterTextSplitter::new(200)
        .with_chunk_overlap(20);
    let chunks = splitter.split_documents(&documents);

    // 3. 创建嵌入模型和向量存储
    let embeddings = FakeEmbeddings::new();
    let store = InMemoryVectorStore::new();
    store.add_documents(chunks, &embeddings).await?;

    // 4. 检索
    let retriever = VectorStoreRetriever::new(store, embeddings, 2);
    let relevant_docs = retriever.retrieve("Synaptic 是什么？").await?;

    // 5. 生成（这里使用伪代码，实际使用你的 ChatModel）
    let context = relevant_docs.iter()
        .map(|doc| doc.content.as_str())
        .collect::<Vec<_>>()
        .join("\n\n");

    let messages = vec![
        Message::system(&format!("根据以下上下文回答问题：\n{}", context)),
        Message::human("Synaptic 是什么？"),
    ];

    let request = ChatRequest::new(messages);
    // let response = model.chat(request).await?;
    // println!("{}", response.message.content());

    Ok(())
}
```

## 总结

在本教程中你学会了：

- 使用文档加载器从各种来源加载文档
- 使用 `RecursiveCharacterTextSplitter` 将长文档拆分为适合检索的小块
- 使用嵌入模型和 `InMemoryVectorStore` 建立向量索引
- 使用 `VectorStoreRetriever` 检索相关文档
- 将检索到的上下文与提示结合，发送给 LLM 生成回答

## 下一步

- [构建 Graph 工作流](graph-workflow.md) -- 使用状态机编排复杂的 RAG 流程
- [构建 ReAct Agent](react-agent.md) -- 将检索与工具调用结合
- [构建带记忆的聊天机器人](chatbot-with-memory.md) -- 为 RAG 应用添加对话记忆
