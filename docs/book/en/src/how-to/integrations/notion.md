# Notion Loader

Load content from Notion pages into Synaptic documents using the Notion API.

## Setup

```toml
[dependencies]
synaptic = { version = "0.2", features = ["loaders"] }
```

Create an integration at [notion.so/my-integrations](https://www.notion.so/my-integrations) and get an Internal Integration Token. Share the page with your integration.

## Usage

```rust,ignore
use synaptic::loaders::NotionLoader;
use synaptic::core::Loader;

let loader = NotionLoader::new("secret_your_token", vec![
    "page-id-1".to_string(),
    "page-id-2".to_string(),
]);

let docs = loader.load().await?;
for doc in &docs {
    println!("Page: {}", doc.metadata["title"]);
    println!("Content: {}", &doc.content[..200.min(doc.content.len())]);
}
```

## Metadata Fields

Each document includes:

- `source` — `notion:<page-id>`
- `title` — the page title extracted from page properties

## Supported Block Types

Paragraphs, headings (H1/H2/H3), bullet lists, numbered lists, quotes, callouts, and code blocks are extracted. Other blocks (images, embeds, databases, etc.) are skipped.
