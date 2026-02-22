# GitHub Loader

Load source code files and documentation from GitHub repositories via the GitHub Contents API.

## Setup

```toml
[dependencies]
synaptic = { version = "0.2", features = ["loaders"] }
```

## Usage

```rust,ignore
use synaptic::loaders::GitHubLoader;
use synaptic::core::Loader;

// Load a single file
let loader = GitHubLoader::new("rust-lang", "rust", vec!["README.md".to_string()])
    .with_token("ghp_your_token")
    .with_branch("master");

// Load all .rs files in a directory (recursive)
let loader = GitHubLoader::new("owner", "repo", vec!["src/".to_string()])
    .with_token("ghp_your_token")
    .with_extensions(vec![".rs".to_string(), ".md".to_string()]);

let docs = loader.load().await?;
for doc in &docs {
    println!("File: {}", doc.id);
    println!("Source: {}", doc.metadata["source"]);
}
```

## Configuration

| Method | Description |
|--------|-------------|
| `with_token(token)` | GitHub personal access token (for private repos or higher rate limits) |
| `with_branch(branch)` | Specific branch/tag/commit SHA to load from |
| `with_extensions(exts)` | Filter files by extension (e.g. `[".rs", ".md"]`); empty = all files |

## Metadata Fields

Each document includes:

- `source` — `github:<owner>/<repo>/<path>`
- `sha` — the file's git blob SHA
- `branch` — branch name (if specified)
