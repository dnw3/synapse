# PDF Loader

This guide shows how to load documents from PDF files using Synaptic's `PdfLoader`. It extracts text content from PDFs and produces `Document` values that can be passed to text splitters, embeddings, and vector stores.

## Setup

Add the `pdf` feature to your `Cargo.toml`:

```toml
[dependencies]
synaptic = { version = "0.3", features = ["pdf"] }
```

The PDF extraction is handled by the `pdf_extract` library, which is pulled in automatically.

## Loading a PDF as a single document

By default, `PdfLoader` combines all pages into one `Document`:

```rust,ignore
use synaptic::pdf::{PdfLoader, Loader};

let loader = PdfLoader::new("report.pdf");
let docs = loader.load().await?;

assert_eq!(docs.len(), 1);
println!("Content: {}", docs[0].content);
println!("Source: {}", docs[0].metadata["source"]);       // "report.pdf"
println!("Pages: {}", docs[0].metadata["total_pages"]);   // e.g. 12
```

The document ID is set to the file path string. Metadata includes:

- `source` -- the file path
- `total_pages` -- the total number of pages in the PDF

## Loading with one document per page

Use `with_split_pages` to produce a separate `Document` for each page:

```rust,ignore
use synaptic::pdf::{PdfLoader, Loader};

let loader = PdfLoader::with_split_pages("report.pdf");
let docs = loader.load().await?;

for doc in &docs {
    println!(
        "Page {}/{}: {}...",
        doc.metadata["page"],
        doc.metadata["total_pages"],
        &doc.content[..80]
    );
}
```

Each document has the following metadata:

- `source` -- the file path
- `page` -- the 1-based page number
- `total_pages` -- the total number of pages

Document IDs follow the format `{path}:page_{n}` (e.g. `report.pdf:page_3`). Empty pages are automatically skipped.

## RAG pipeline with PDF

A common pattern is to load a PDF, split it into chunks, embed, and store for retrieval:

```rust,ignore
use synaptic::pdf::{PdfLoader, Loader};
use synaptic::splitters::{RecursiveCharacterTextSplitter, TextSplitter};
use synaptic::vectorstores::{InMemoryVectorStore, VectorStore, VectorStoreRetriever};
use synaptic::openai::OpenAiEmbeddings;
use synaptic::retrieval::Retriever;
use std::sync::Arc;

// 1. Load the PDF
let loader = PdfLoader::with_split_pages("manual.pdf");
let docs = loader.load().await?;

// 2. Split into chunks
let splitter = RecursiveCharacterTextSplitter::new(1000, 200);
let chunks = splitter.split_documents(&docs)?;

// 3. Embed and store
let embeddings = Arc::new(OpenAiEmbeddings::new("text-embedding-3-small"));
let store = Arc::new(InMemoryVectorStore::new());
store.add_documents(chunks, embeddings.as_ref()).await?;

// 4. Retrieve
let retriever = VectorStoreRetriever::new(store, embeddings, 5);
let results = retriever.retrieve("How do I configure the system?", 5).await?;
```

This works equally well with `QdrantVectorStore` or `PgVectorStore` in place of `InMemoryVectorStore`.

## Processing multiple PDFs

Use `DirectoryLoader` with a glob filter, or load PDFs individually and merge the results:

```rust,ignore
use synaptic::pdf::{PdfLoader, Loader};

let paths = vec!["docs/intro.pdf", "docs/guide.pdf", "docs/reference.pdf"];

let mut all_docs = Vec::new();
for path in paths {
    let loader = PdfLoader::with_split_pages(path);
    let docs = loader.load().await?;
    all_docs.extend(docs);
}
// all_docs now contains page-level documents from all three PDFs
```

## How text extraction works

`PdfLoader` uses the `pdf_extract` library internally. Text extraction runs on a blocking thread via `tokio::task::spawn_blocking` to avoid blocking the async runtime.

Page boundaries are detected by form feed characters (`\x0c`) that `pdf_extract` inserts between pages. When using `with_split_pages`, the text is split on these characters and each non-empty segment becomes a document.

## Configuration reference

| Constructor | Behavior |
|-------------|----------|
| `PdfLoader::new(path)` | All pages combined into a single `Document` |
| `PdfLoader::with_split_pages(path)` | One `Document` per page |

### Metadata fields

| Field | Type | Present in | Description |
|-------|------|------------|-------------|
| `source` | `String` | Both modes | The file path |
| `page` | `Number` | Split pages only | 1-based page number |
| `total_pages` | `Number` | Both modes | Total number of pages in the PDF |
