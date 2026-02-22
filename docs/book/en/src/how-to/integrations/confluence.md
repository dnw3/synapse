# Confluence Loader

Load Confluence wiki pages into Synaptic documents using the Confluence REST API v2.

## Setup

```toml
[dependencies]
synaptic = { version = "0.2", features = ["confluence"] }
```

Create an API token at [id.atlassian.com/manage-profile/security/api-tokens](https://id.atlassian.com/manage-profile/security/api-tokens).

## Usage

```rust,ignore
use synaptic::confluence::{ConfluenceConfig, ConfluenceLoader};
use synaptic::core::Loader;

// Load specific pages by ID
let config = ConfluenceConfig::new(
    "yourcompany.atlassian.net",
    "you@example.com",
    "your-api-token",
)
.with_page_ids(vec!["12345678".to_string(), "87654321".to_string()]);
let loader = ConfluenceLoader::new(config);

// Or load all pages in a space
let config = ConfluenceConfig::new(
    "yourcompany.atlassian.net",
    "you@example.com",
    "your-api-token",
)
.with_space_key("ENGDOCS");
let loader = ConfluenceLoader::new(config);

let docs = loader.load().await?;
for doc in &docs {
    println!("Title: {}", doc.metadata["title"]);
    println!("Content: {}", &doc.content[..200.min(doc.content.len())]);
}
```

## Metadata Fields

Each document includes:

- `source` — `confluence:<page-id>`
- `title` — the page title
- `space_id` — the space identifier (if available)

## Notes

HTML storage format is stripped to plain text. Pages that fail to load emit a warning and are skipped rather than failing the entire load operation.
