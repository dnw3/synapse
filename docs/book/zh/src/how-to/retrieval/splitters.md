# 文本分割器

本指南展示如何使用 Synaptic 的 `TextSplitter` trait 及其内置实现将大型文档拆分为较小的块。

## 概述

所有分割器都实现了 `synaptic_splitters` 中的 `TextSplitter` trait：

```rust
pub trait TextSplitter: Send + Sync {
    fn split_text(&self, text: &str) -> Vec<String>;
    fn split_documents(&self, docs: Vec<Document>) -> Vec<Document>;
}
```

- `split_text()` 接受一个字符串，返回一个块的向量。
- `split_documents()` 分割每个文档的内容，生成新的 `Document` 值，保留原有元数据并添加 `chunk_index` 字段。

## CharacterTextSplitter

按单个分隔符字符串分割文本，然后合并小片段以保持在 `chunk_size` 以内。

```rust
use synaptic_splitters::CharacterTextSplitter;
use synaptic_splitters::TextSplitter;

// Chunk size in characters, default separator is "\n\n"
let splitter = CharacterTextSplitter::new(500);
let chunks = splitter.split_text("long text...");
```

配置分隔符和重叠：

```rust
let splitter = CharacterTextSplitter::new(500)
    .with_separator("\n")       // Split on single newlines
    .with_chunk_overlap(50);    // 50 characters of overlap between chunks
```

## RecursiveCharacterTextSplitter

最常用的分割器。按顺序尝试一组层级分隔符，使用第一个能产生足够小块的分隔符进行分割。如果某个块仍然过大，则递归使用下一个分隔符。

默认分隔符：`["\n\n", "\n", " ", ""]`

```rust
use synaptic_splitters::RecursiveCharacterTextSplitter;
use synaptic_splitters::TextSplitter;

let splitter = RecursiveCharacterTextSplitter::new(1000)
    .with_chunk_overlap(200);

let chunks = splitter.split_text("long document text...");
```

自定义分隔符：

```rust
let splitter = RecursiveCharacterTextSplitter::new(1000)
    .with_separators(vec![
        "\n\n\n".to_string(),
        "\n\n".to_string(),
        "\n".to_string(),
        " ".to_string(),
        String::new(),
    ]);
```

### 语言感知分割

使用 `from_language()` 获取针对特定编程语言调优的分隔符：

```rust
use synaptic_splitters::{RecursiveCharacterTextSplitter, Language};

let splitter = RecursiveCharacterTextSplitter::from_language(
    Language::Rust,
    1000,  // chunk_size
    200,   // chunk_overlap
);
```

## MarkdownHeaderTextSplitter

按标题分割 Markdown 文本，将标题层级添加到每个块的元数据中。

```rust
use synaptic_splitters::{MarkdownHeaderTextSplitter, HeaderType};

let splitter = MarkdownHeaderTextSplitter::new(vec![
    HeaderType { level: "#".to_string(), name: "h1".to_string() },
    HeaderType { level: "##".to_string(), name: "h2".to_string() },
    HeaderType { level: "###".to_string(), name: "h3".to_string() },
]);

let docs = splitter.split_markdown("# Title\n\nIntro text\n\n## Section\n\nBody text");
// docs[0].metadata contains {"h1": "Title"}
// docs[1].metadata contains {"h1": "Title", "h2": "Section"}
```

便捷构造函数提供了默认的 `#`、`##`、`###` 配置：

```rust
let splitter = MarkdownHeaderTextSplitter::default_headers();
```

注意 `MarkdownHeaderTextSplitter` 也实现了 `TextSplitter`，但 `split_markdown()` 返回带有完整元数据的 `Vec<Document>`，这通常是你需要的。

## TokenTextSplitter

使用约 4 字符/Token 的启发式方法按估计的 Token 数量分割文本。在单词边界处分割以保持块的可读性。

```rust
use synaptic_splitters::TokenTextSplitter;
use synaptic_splitters::TextSplitter;

// chunk_size is in estimated tokens (not characters)
let splitter = TokenTextSplitter::new(500)
    .with_chunk_overlap(50);

let chunks = splitter.split_text("long text...");
```

这与 `ConversationTokenBufferMemory` 中使用的 Token 估算方式一致。

## HtmlHeaderTextSplitter

按 HTML 标题标签（`<h1>`、`<h2>` 等）分割 HTML 文本，将标题层级添加到每个块的元数据中。类似于 `MarkdownHeaderTextSplitter`，但用于 HTML 内容。

```rust
use synaptic_splitters::HtmlHeaderTextSplitter;

let splitter = HtmlHeaderTextSplitter::new(vec![
    ("h1".to_string(), "Header 1".to_string()),
    ("h2".to_string(), "Header 2".to_string()),
]);

let html = "<h1>Title</h1><p>Intro text</p><h2>Section</h2><p>Body text</p>";
let docs = splitter.split_html(html);
// docs[0].metadata contains {"Header 1": "Title"}
// docs[1].metadata contains {"Header 1": "Title", "Header 2": "Section"}
```

构造函数接受一个 `(tag_name, metadata_key)` 对的列表。只有指定的标签会被视为分割点；其他所有 HTML 内容都被视为当前节的正文文本。

## 分割文档

所有分割器都可以将 `Vec<Document>` 分割为更小的块。每个块继承父文档的元数据，并获得一个 `chunk_index` 字段。块的 ID 格式为 `"{original_id}-chunk-{index}"`。

```rust
use synaptic_splitters::{RecursiveCharacterTextSplitter, TextSplitter};
use synaptic_retrieval::Document;

let splitter = RecursiveCharacterTextSplitter::new(500);

let docs = vec![
    Document::new("doc-1", "A very long document..."),
    Document::new("doc-2", "Another long document..."),
];

let chunks = splitter.split_documents(docs);
// chunks[0].id == "doc-1-chunk-0"
// chunks[0].metadata["chunk_index"] == 0
```

## 选择分割器

| 分割器 | 最适合 |
|----------|----------|
| `CharacterTextSplitter` | 按已知分隔符进行简单分割 |
| `RecursiveCharacterTextSplitter` | 通用文本 -- 尝试保留段落、然后句子、然后单词 |
| `MarkdownHeaderTextSplitter` | 需要在元数据中保留标题上下文的 Markdown 文档 |
| `TokenTextSplitter` | 需要按 Token 数量而非字符数量控制块大小时 |
