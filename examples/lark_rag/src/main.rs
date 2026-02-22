//! Lark RAG example: load a Feishu Wiki, index into LarkVectorStore, answer questions.
//!
//! Required env vars:
//!   LARK_APP_ID        — Lark application ID (e.g. "cli_xxx")
//!   LARK_APP_SECRET    — Lark application secret
//!   LARK_WIKI_SPACE_ID — Wiki space ID to load
//!   LARK_DATASET_ID    — Lark Search dataset ID to index into
//!
//! Run (requires live Lark credentials):
//!   LARK_APP_ID=cli_xxx \
//!   LARK_APP_SECRET=secret \
//!   LARK_WIKI_SPACE_ID=space_xxx \
//!   LARK_DATASET_ID=dataset_xxx \
//!   cargo run -p lark_rag

use synaptic::core::{Loader, VectorStore};
use synaptic::embeddings::FakeEmbeddings;
use synaptic::lark::{LarkConfig, LarkVectorStore, LarkWikiLoader};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let app_id = std::env::var("LARK_APP_ID").unwrap_or_else(|_| "cli_placeholder".to_string());
    let app_secret =
        std::env::var("LARK_APP_SECRET").unwrap_or_else(|_| "secret_placeholder".to_string());
    let space_id =
        std::env::var("LARK_WIKI_SPACE_ID").unwrap_or_else(|_| "space_placeholder".to_string());
    let dataset_id =
        std::env::var("LARK_DATASET_ID").unwrap_or_else(|_| "dataset_placeholder".to_string());

    let config = LarkConfig::new(&app_id, &app_secret);

    // ── Step 1: Load wiki documents ──────────────────────────────────────────
    println!("Loading wiki space '{space_id}'...");
    let loader = LarkWikiLoader::new(config.clone())
        .with_space_id(&space_id)
        .with_max_depth(3);
    let docs = loader.load().await?;
    println!("Loaded {} document(s).", docs.len());

    if docs.is_empty() {
        println!("No documents found. Check LARK_WIKI_SPACE_ID and credentials.");
        return Ok(());
    }

    // ── Step 2: Index into LarkVectorStore ───────────────────────────────────
    // LarkVectorStore is backed by the Lark Search API, which performs
    // server-side full-text search.  The Embeddings argument is accepted by
    // the VectorStore trait but is not used by Lark (it uses its own indexing).
    println!(
        "Indexing {} document(s) into dataset '{dataset_id}'...",
        docs.len()
    );
    let store = LarkVectorStore::new(config, &dataset_id);
    // LarkVectorStore ignores the embedder (Lark uses server-side indexing);
    // any dimension value is fine here.
    let embeddings = FakeEmbeddings::new(1536);
    let ids = store.add_documents(docs, &embeddings).await?;
    println!(
        "Indexed {} document(s): {:?}",
        ids.len(),
        &ids[..ids.len().min(5)]
    );

    // ── Step 3: Search ───────────────────────────────────────────────────────
    let query = "年假申请流程";
    println!("\nSearching for: '{query}'");
    let results = store.similarity_search(query, 3, &embeddings).await?;

    if results.is_empty() {
        println!("No results returned.");
    } else {
        for (i, doc) in results.iter().enumerate() {
            let snippet = &doc.content[..doc.content.len().min(200)];
            println!("\n[{}] id={}\n{}", i + 1, doc.id, snippet);
        }
    }

    Ok(())
}
