use synaptic_splitters::{
    CharacterTextSplitter, Document, RecursiveCharacterTextSplitter, TextSplitter,
    TokenTextSplitter,
};

// --- CharacterTextSplitter edge cases ---

#[test]
fn character_splitter_empty_input() {
    let splitter = CharacterTextSplitter::new(100);
    let result = splitter.split_text("");
    // Empty text has one split (the empty string itself) which yields one chunk
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], "");
}

#[test]
fn character_splitter_text_shorter_than_chunk() {
    let splitter = CharacterTextSplitter::new(1000);
    let result = splitter.split_text("short text");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], "short text");
}

#[test]
fn character_splitter_separator_not_found() {
    // Use a separator that does not exist in the text. The text is treated
    // as a single split and returned as-is (even if it exceeds chunk_size,
    // because there is nothing to split on).
    let splitter = CharacterTextSplitter::new(10).with_separator("|||");
    let result = splitter.split_text("hello world this is a test");
    assert!(!result.is_empty());
    // With the separator missing, the entire text is one split
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], "hello world this is a test");
}

#[test]
fn character_splitter_overlap_larger_than_splits() {
    // Overlap that is larger than individual splits should not panic
    let splitter = CharacterTextSplitter::new(10)
        .with_separator(" ")
        .with_chunk_overlap(8);
    let text = "aa bb cc dd ee";
    let result = splitter.split_text(text);
    assert!(!result.is_empty());
    // All original content should be present across chunks
    for word in &["aa", "bb", "cc", "dd", "ee"] {
        assert!(
            result.iter().any(|chunk| chunk.contains(word)),
            "word '{}' missing from chunks: {:?}",
            word,
            result,
        );
    }
}

// --- RecursiveCharacterTextSplitter edge cases ---

#[test]
fn recursive_splitter_hierarchical_fallback() {
    // Text has no double newlines but has single newlines
    let splitter = RecursiveCharacterTextSplitter::new(20);
    let text = "first line\nsecond line\nthird line";
    let result = splitter.split_text(text);
    assert!(result.len() > 1);
    for chunk in &result {
        assert!(
            chunk.len() <= 20,
            "chunk too long: {} chars in '{}'",
            chunk.len(),
            chunk,
        );
    }
}

#[test]
fn recursive_splitter_custom_separators() {
    let splitter = RecursiveCharacterTextSplitter::new(15).with_separators(vec![
        ";".to_string(),
        ",".to_string(),
        "".to_string(),
    ]);
    let text = "alpha;beta,gamma;delta";
    let result = splitter.split_text(text);
    assert!(result.len() > 1);
    for chunk in &result {
        assert!(
            chunk.len() <= 15,
            "chunk too long: {} chars in '{}'",
            chunk.len(),
            chunk,
        );
    }
}

#[test]
fn recursive_splitter_empty_input() {
    let splitter = RecursiveCharacterTextSplitter::new(100);
    let result = splitter.split_text("");
    // Empty text is shorter than chunk_size, returned as single chunk
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], "");
}

// --- TokenTextSplitter edge cases ---

#[test]
fn token_splitter_empty_text() {
    let splitter = TokenTextSplitter::new(100);
    let result = splitter.split_text("");
    // split_whitespace on "" yields no words, so result is empty
    assert!(result.is_empty());
}

#[test]
fn token_splitter_single_word() {
    let splitter = TokenTextSplitter::new(100);
    let result = splitter.split_text("hello");
    assert_eq!(result.len(), 1);
    assert_eq!(result[0], "hello");
}

#[test]
fn token_splitter_with_overlap() {
    let splitter = TokenTextSplitter::new(5).with_chunk_overlap(2);
    let text = "The quick brown fox jumped over the lazy dog and other animals in the park";
    let result = splitter.split_text(text);
    assert!(result.len() > 1, "should produce multiple chunks");

    // Verify overlap: at least some content should appear in consecutive chunks
    for window in result.windows(2) {
        let words_a: Vec<&str> = window[0].split_whitespace().collect();
        let words_b: Vec<&str> = window[1].split_whitespace().collect();
        // With overlap, the last word(s) of chunk A should appear in chunk B
        let has_overlap = words_a.iter().any(|w| words_b.contains(w));
        assert!(
            has_overlap,
            "expected overlap between consecutive chunks: '{}' and '{}'",
            window[0], window[1],
        );
    }
}

// --- split_documents preserves metadata ---

#[test]
fn split_documents_preserves_metadata() {
    let splitter = CharacterTextSplitter::new(10).with_separator(" ");
    let doc = Document::with_metadata(
        "doc1",
        "hello world how are you today",
        [("source".to_string(), serde_json::json!("test.txt"))].into(),
    );
    let result = splitter.split_documents(vec![doc]);
    assert!(result.len() > 1, "should split into multiple docs");
    for d in &result {
        assert_eq!(
            d.metadata.get("source").unwrap(),
            "test.txt",
            "metadata 'source' should be preserved on chunk '{}'",
            d.id,
        );
        assert!(
            d.metadata.contains_key("chunk_index"),
            "chunk_index metadata should be added",
        );
    }
}
