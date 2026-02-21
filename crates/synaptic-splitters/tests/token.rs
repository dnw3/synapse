use synaptic_splitters::{Document, TextSplitter, TokenTextSplitter};

#[test]
fn basic_split_by_token_count() {
    // chunk_size=10 tokens => ~40 chars per chunk
    let splitter = TokenTextSplitter::new(10);
    let text = "The quick brown fox jumps over the lazy dog and runs through the field at dawn";
    let chunks = splitter.split_text(text);

    assert!(chunks.len() > 1);
    for chunk in &chunks {
        // Each chunk should be roughly within the token budget
        let est_tokens = (chunk.len() / 4).max(1);
        assert!(
            est_tokens <= 12,
            "chunk too large ({est_tokens} tokens): {chunk}"
        );
    }
}

#[test]
fn split_with_overlap() {
    let splitter = TokenTextSplitter::new(5).with_chunk_overlap(2);
    let text = "one two three four five six seven eight nine ten";
    let chunks = splitter.split_text(text);

    assert!(chunks.len() > 1);
    // Chunks with overlap should share some words
}

#[test]
fn split_documents() {
    let splitter = TokenTextSplitter::new(5);
    let docs = vec![Document::new("doc1", "word ".repeat(20).trim())];
    let chunks = splitter.split_documents(docs);

    assert!(chunks.len() > 1);
    assert!(chunks[0].id.starts_with("doc1-chunk-"));
    assert!(chunks[0].metadata.contains_key("chunk_index"));
}

#[test]
fn small_text_single_chunk() {
    let splitter = TokenTextSplitter::new(100);
    let chunks = splitter.split_text("hello world");
    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0], "hello world");
}
