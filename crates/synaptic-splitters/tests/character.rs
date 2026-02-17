use synaptic_splitters::{CharacterTextSplitter, TextSplitter};

#[test]
fn splits_by_double_newline() {
    let splitter = CharacterTextSplitter::new(50);
    let text = "First paragraph.\n\nSecond paragraph.\n\nThird paragraph.";
    let chunks = splitter.split_text(text);

    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0], "First paragraph.\n\nSecond paragraph.");
    assert_eq!(chunks[1], "Third paragraph.");
}

#[test]
fn custom_separator() {
    let splitter = CharacterTextSplitter::new(20).with_separator(", ");
    let text = "apple, banana, cherry, date";
    let chunks = splitter.split_text(text);

    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0], "apple, banana");
    assert_eq!(chunks[1], "cherry, date");
}

#[test]
fn respects_chunk_overlap() {
    let splitter = CharacterTextSplitter::new(20)
        .with_separator(" ")
        .with_chunk_overlap(5);
    let text = "one two three four five";
    let chunks = splitter.split_text(text);

    assert!(chunks.len() >= 2);
    // Overlap means some words appear in adjacent chunks
}

#[test]
fn small_text_returns_single_chunk() {
    let splitter = CharacterTextSplitter::new(100);
    let text = "Small text.";
    let chunks = splitter.split_text(text);

    assert_eq!(chunks.len(), 1);
    assert_eq!(chunks[0], "Small text.");
}
