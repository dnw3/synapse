# 文档加载器

本指南展示如何使用 Synaptic 的 `Loader` trait 及其内置实现从各种来源加载文档。

## 概述

每个加载器都实现了 `synaptic_loaders` 中的 `Loader` trait：

```rust
#[async_trait]
pub trait Loader: Send + Sync {
    async fn load(&self) -> Result<Vec<Document>, SynapticError>;
}
```

每个加载器返回 `Vec<Document>`。一个 `Document` 包含三个字段：

- `id: String` -- 唯一标识符
- `content: String` -- 文档文本
- `metadata: HashMap<String, Value>` -- 任意键值对元数据

## TextLoader

将一段文本字符串包装为单个 `Document`。适用于内容已经在内存中的场景。

```rust
use synaptic_loaders::{TextLoader, Loader};

let loader = TextLoader::new("doc-1", "Rust is a systems programming language.");
let docs = loader.load().await?;

assert_eq!(docs.len(), 1);
assert_eq!(docs[0].content, "Rust is a systems programming language.");
```

第一个参数是文档 ID；第二个是内容。

## FileLoader

使用 `tokio::fs::read_to_string` 从磁盘读取文件，返回单个 `Document`。文件路径用作文档 ID，并将 `source` 元数据键设置为文件路径。

```rust
use synaptic_loaders::{FileLoader, Loader};

let loader = FileLoader::new("data/notes.txt");
let docs = loader.load().await?;

assert_eq!(docs[0].metadata["source"], "data/notes.txt");
```

## JsonLoader

从 JSON 字符串加载文档。如果 JSON 是对象数组，则每个对象成为一个 `Document`。如果是单个对象，则生成一个 `Document`。

```rust
use synaptic_loaders::{JsonLoader, Loader};

let json_data = r#"[
    {"id": "1", "content": "First document"},
    {"id": "2", "content": "Second document"}
]"#;

let loader = JsonLoader::new(json_data);
let docs = loader.load().await?;

assert_eq!(docs.len(), 2);
assert_eq!(docs[0].content, "First document");
```

默认情况下，`JsonLoader` 查找 `"id"` 和 `"content"` 键。你可以通过构建器方法自定义它们：

```rust
let loader = JsonLoader::new(json_data)
    .with_id_key("doc_id")
    .with_content_key("text");
```

## CsvLoader

从 CSV 数据加载文档。每行成为一个 `Document`。所有列都存储为元数据。

```rust
use synaptic_loaders::{CsvLoader, Loader};

let csv_data = "title,body,author\nIntro,Hello world,Alice\nChapter 1,Once upon a time,Bob";

let loader = CsvLoader::new(csv_data)
    .with_content_column("body")
    .with_id_column("title");

let docs = loader.load().await?;

assert_eq!(docs.len(), 2);
assert_eq!(docs[0].id, "Intro");
assert_eq!(docs[0].content, "Hello world");
assert_eq!(docs[0].metadata["author"], "Alice");
```

如果未指定 `content_column`，则所有列会被拼接。如果未指定 `id_column`，ID 默认为 `"row-0"`、`"row-1"` 等。

## DirectoryLoader

从目录加载所有文件，每个文件成为一个 `Document`。使用 `with_glob` 按扩展名过滤，使用 `with_recursive` 包含子目录。

```rust
use synaptic_loaders::{DirectoryLoader, Loader};

let loader = DirectoryLoader::new("./docs")
    .with_glob("*.txt")
    .with_recursive(true);

let docs = loader.load().await?;
// Each document has a `source` metadata key set to the file path
```

文档 ID 是相对于基础目录的相对文件路径。

## MarkdownLoader

读取一个 Markdown 文件，返回单个 `Document`，元数据中包含 `format: "markdown"`。

```rust
use synaptic_loaders::{MarkdownLoader, Loader};

let loader = MarkdownLoader::new("docs/guide.md");
let docs = loader.load().await?;

assert_eq!(docs[0].metadata["format"], "markdown");
```

## WebBaseLoader

通过 HTTP GET 从 URL 获取内容，返回单个 `Document`。元数据包括 `source`（URL）和 `content_type`（来自响应头）。

```rust
use synaptic_loaders::{WebBaseLoader, Loader};

let loader = WebBaseLoader::new("https://example.com/page.html");
let docs = loader.load().await?;

assert_eq!(docs[0].metadata["source"], "https://example.com/page.html");
```

## 惰性加载

每个 `Loader` 还提供了一个 `lazy_load()` 方法，返回文档的 `Stream` 而不是一次性全部加载。默认实现封装了 `load()`，但自定义加载器可以覆盖它以实现真正的惰性行为。

```rust
use futures::StreamExt;
use synaptic_loaders::{DirectoryLoader, Loader};

let loader = DirectoryLoader::new("./data").with_glob("*.txt");
let mut stream = loader.lazy_load();

while let Some(result) = stream.next().await {
    let doc = result?;
    println!("Loaded: {}", doc.id);
}
```
