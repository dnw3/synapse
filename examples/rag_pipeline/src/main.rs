use std::sync::Arc;

use synaptic::core::SynapticError;
use synaptic::embeddings::FakeEmbeddings;
use synaptic::loaders::{Loader, TextLoader};
use synaptic::retrieval::Retriever;
use synaptic::splitters::{RecursiveCharacterTextSplitter, TextSplitter};
use synaptic::vectorstores::{InMemoryVectorStore, VectorStore, VectorStoreRetriever};

#[tokio::main]
async fn main() -> Result<(), SynapticError> {
    let embeddings = FakeEmbeddings::default();

    // --- Load documents ---
    println!("=== Loading Documents ===");
    let loader1 = TextLoader::new("doc1", "Rust is a systems programming language focused on safety, speed, and concurrency. It achieves memory safety without garbage collection.");
    let loader2 = TextLoader::new("doc2", "Python is a high-level programming language known for its simplicity and readability. It is widely used in data science and machine learning.");
    let loader3 = TextLoader::new("doc3", "TypeScript is a typed superset of JavaScript that compiles to plain JavaScript. It adds optional static typing to the language.");

    let mut docs = Vec::new();
    for loader in [loader1, loader2, loader3] {
        docs.extend(loader.load().await?);
    }
    println!("Loaded {} documents", docs.len());

    // --- Split documents ---
    println!("\n=== Splitting Documents ===");
    let splitter = RecursiveCharacterTextSplitter::new(80).with_chunk_overlap(10);
    let chunks = splitter.split_documents(docs);
    println!("Split into {} chunks", chunks.len());
    for (i, chunk) in chunks.iter().enumerate() {
        println!(
            "  chunk {i}: \"{}...\"",
            &chunk.content[..chunk.content.len().min(50)]
        );
    }

    // --- Build vector store ---
    println!("\n=== Building Vector Store ===");
    let store = InMemoryVectorStore::from_documents(chunks, &embeddings).await?;
    println!("Vector store built");

    // --- Similarity search ---
    println!("\n=== Similarity Search ===");
    let results = store
        .similarity_search("memory safety", 2, &embeddings)
        .await?;
    println!("Top 2 results for 'memory safety':");
    for (i, doc) in results.iter().enumerate() {
        println!("  {i}: \"{}\"", doc.content);
    }

    // --- Retriever interface ---
    println!("\n=== Retriever ===");
    let retriever = VectorStoreRetriever::new(Arc::new(store), Arc::new(embeddings), 2);
    let docs = retriever.retrieve("type safety", 2).await?;
    println!("Retrieved {} docs for 'type safety'", docs.len());
    for doc in &docs {
        println!("  \"{}\"", doc.content);
    }

    println!("\nRAG pipeline demo completed successfully!");
    Ok(())
}
