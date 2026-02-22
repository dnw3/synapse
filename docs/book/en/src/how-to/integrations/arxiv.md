# arXiv Loader

Load academic papers from arXiv as Synaptic documents. Returns paper abstracts with title, authors, and publication date metadata.

## Setup

```toml
[dependencies]
synaptic = { version = "0.2", features = ["loaders"] }
```

No API key required — arXiv provides a free public API.

## Usage

```rust,ignore
use synaptic::loaders::ArxivLoader;
use synaptic::core::Loader;

let loader = ArxivLoader::new("large language models rust")
    .with_max_results(10);

let docs = loader.load().await?;
for doc in &docs {
    println!("Title: {}", doc.metadata["title"]);
    println!("Authors: {}", doc.metadata["authors"]);
    println!("Published: {}", doc.metadata["published"]);
    println!("Abstract: {}", &doc.content[..200.min(doc.content.len())]);
}
```

## Metadata Fields

Each document includes:

- `source` — `arxiv:<arxiv-id>`
- `url` — `https://arxiv.org/abs/<arxiv-id>`
- `title` — paper title
- `authors` — comma-separated author names
- `published` — ISO 8601 publication date

## Notes

Results are sorted by submission date (newest first). The `doc.content` field contains the abstract text. The arXiv API has rate limits; add delays between requests for large batches.
