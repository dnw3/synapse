# PDF 文档加载器

本指南展示如何使用 Synaptic 的 PDF 集成从 PDF 文件中加载文档内容。

## 概述

`synaptic_pdf` crate 提供了 `PdfLoader`，它实现了 `Loader` trait，可以从 PDF 文件中提取文本内容。支持两种加载模式：

- **整文档模式** -- 将所有页面合并为一个 `Document`
- **按页拆分模式** -- 每页生成一个独立的 `Document`

## Cargo.toml 配置

```toml
[dependencies]
synaptic = { version = "0.3", features = ["pdf"] }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

## 基础使用

### 整文档加载

将 PDF 的所有页面合并为单个 `Document`：

```rust,ignore
use synaptic::pdf::PdfLoader;
use synaptic::loaders::Loader;

let loader = PdfLoader::new("path/to/document.pdf");
let docs = loader.load().await?;

assert_eq!(docs.len(), 1);
println!("内容长度: {} 字符", docs[0].content.len());
```

所有页面的文本按顺序拼接。文档 ID 使用文件路径。

### 按页拆分加载

每页生成一个独立的 `Document`，方便精细化的检索和引用：

```rust,ignore
use synaptic::pdf::PdfLoader;
use synaptic::loaders::Loader;

let loader = PdfLoader::with_split_pages("path/to/document.pdf");
let docs = loader.load().await?;

println!("共加载 {} 页", docs.len());
for doc in &docs {
    println!("页码 {}: {} 字符",
        doc.metadata["page"],
        doc.content.len(),
    );
}
```

按页拆分时，每个 `Document` 的 `metadata` 包含 `page` 字段（页码，从 1 开始）。

## 配置选项

### 加载模式选择

| 方法 | 文档数量 | 适用场景 |
|------|---------|---------|
| `PdfLoader::new(path)` | 1 个 | 需要全文搜索或整体摘要 |
| `PdfLoader::with_split_pages(path)` | 每页 1 个 | 需要按页检索或引用具体页码 |

## 常见模式

### 加载后分割

对于长篇 PDF，先整文档加载，再用文本分割器切分为适合 LLM 上下文窗口的小块：

```rust,ignore
use synaptic::pdf::PdfLoader;
use synaptic::loaders::Loader;
use synaptic::splitters::{RecursiveCharacterTextSplitter, TextSplitter};

let loader = PdfLoader::new("research_paper.pdf");
let docs = loader.load().await?;

let splitter = RecursiveCharacterTextSplitter::new(1000, 100);
let chunks = splitter.split_documents(&docs)?;

println!("PDF 被分割为 {} 个文本块", chunks.len());
```

### 按页加载后分割

先按页拆分加载，再对每页内容进一步分割。这样每个 chunk 都保留了页码元数据：

```rust,ignore
use synaptic::pdf::PdfLoader;
use synaptic::loaders::Loader;
use synaptic::splitters::{RecursiveCharacterTextSplitter, TextSplitter};

let loader = PdfLoader::with_split_pages("long_report.pdf");
let pages = loader.load().await?;

let splitter = RecursiveCharacterTextSplitter::new(500, 50);
let chunks = splitter.split_documents(&pages)?;

// 每个 chunk 继承其所在页面的 metadata（包括 page 字段）
for chunk in &chunks {
    println!("来自第 {} 页: {}...",
        chunk.metadata["page"],
        &chunk.content[..50.min(chunk.content.len())],
    );
}
```

### 完整的 PDF RAG 流水线

从 PDF 加载到向量存储和检索的完整流程：

```rust,ignore
use std::sync::Arc;
use synaptic::pdf::PdfLoader;
use synaptic::loaders::Loader;
use synaptic::splitters::{RecursiveCharacterTextSplitter, TextSplitter};
use synaptic::vectorstores::{InMemoryVectorStore, VectorStore, VectorStoreRetriever};
use synaptic::embeddings::OpenAiEmbeddings;
use synaptic::retrieval::Retriever;

// 1. 加载 PDF
let loader = PdfLoader::with_split_pages("knowledge_base.pdf");
let pages = loader.load().await?;

// 2. 分割文本
let splitter = RecursiveCharacterTextSplitter::new(500, 50);
let chunks = splitter.split_documents(&pages)?;

// 3. 存入向量库
let embeddings = Arc::new(OpenAiEmbeddings::new("text-embedding-3-small"));
let store = Arc::new(InMemoryVectorStore::new());
store.add_documents(chunks, embeddings.as_ref()).await?;

// 4. 检索
let retriever = VectorStoreRetriever::new(store, embeddings, 5);
let results = retriever.retrieve("核心概念", 5).await?;

for doc in &results {
    println!("[第 {} 页] {}",
        doc.metadata.get("page").map(|v| v.to_string()).unwrap_or_default(),
        &doc.content[..80.min(doc.content.len())],
    );
}
```

### 批量加载多个 PDF

结合 `DirectoryLoader` 或手动遍历来加载目录下的所有 PDF 文件：

```rust,ignore
use synaptic::pdf::PdfLoader;
use synaptic::loaders::Loader;
use synaptic::retrieval::Document;

let pdf_paths = vec![
    "docs/manual.pdf",
    "docs/api_reference.pdf",
    "docs/tutorial.pdf",
];

let mut all_docs: Vec<Document> = Vec::new();
for path in pdf_paths {
    let loader = PdfLoader::with_split_pages(path);
    let docs = loader.load().await?;
    all_docs.extend(docs);
}

println!("共加载 {} 个文档（来自 {} 个 PDF 文件）",
    all_docs.len(),
    pdf_paths.len(),
);
```

### 与其他加载器的对比

| 加载器 | 输入格式 | 适用场景 |
|--------|---------|---------|
| `TextLoader` | 纯文本字符串 | 内存中的文本 |
| `FileLoader` | 文本文件 | `.txt`、`.md` 等文本文件 |
| `JsonLoader` | JSON | 结构化数据 |
| `CsvLoader` | CSV | 表格数据 |
| `PdfLoader` | PDF | 报告、论文、手册等排版文档 |
