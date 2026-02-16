use synapse_splitters::{RecursiveCharacterTextSplitter, TextSplitter};

#[test]
fn splits_paragraphs_first() {
    let splitter = RecursiveCharacterTextSplitter::new(50);
    let text = "Short paragraph.\n\nAnother short paragraph.\n\nThird one.";
    let chunks = splitter.split_text(text);

    assert!(chunks.len() >= 2);
    for chunk in &chunks {
        assert!(chunk.len() <= 50, "chunk too long: {} chars", chunk.len());
    }
}

#[test]
fn falls_back_to_newlines() {
    let splitter = RecursiveCharacterTextSplitter::new(30);
    let text = "Line one\nLine two\nLine three\nLine four";
    let chunks = splitter.split_text(text);

    assert!(chunks.len() >= 2);
    for chunk in &chunks {
        assert!(chunk.len() <= 30, "chunk too long: {}", chunk.len());
    }
}

#[test]
fn handles_very_long_words() {
    let splitter = RecursiveCharacterTextSplitter::new(10);
    let text = "abcdefghijklmnopqrstuvwxyz";
    let chunks = splitter.split_text(text);

    assert!(chunks.len() >= 2);
    for chunk in &chunks {
        assert!(chunk.len() <= 10);
    }
}

#[test]
fn small_text_single_chunk() {
    let splitter = RecursiveCharacterTextSplitter::new(100);
    let text = "Hello world";
    let chunks = splitter.split_text(text);

    assert_eq!(chunks, vec!["Hello world"]);
}

#[test]
fn split_documents_preserves_metadata() {
    use synapse_retrieval::Document;

    let splitter = RecursiveCharacterTextSplitter::new(20);
    let doc = Document::new(
        "doc-1",
        "Hello world. This is a longer document that should split.",
    );
    let chunks = splitter.split_documents(vec![doc]);

    assert!(chunks.len() >= 2);
    assert!(chunks[0].id.starts_with("doc-1-chunk-"));
    assert!(chunks[0].metadata.contains_key("chunk_index"));
}
