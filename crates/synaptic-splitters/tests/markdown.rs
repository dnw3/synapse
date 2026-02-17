use synaptic_splitters::{MarkdownHeaderTextSplitter, TextSplitter};

#[test]
fn splits_by_headers() {
    let splitter = MarkdownHeaderTextSplitter::default_headers();
    let text = "# Title\n\nIntro text.\n\n## Section 1\n\nContent 1.\n\n## Section 2\n\nContent 2.";
    let docs = splitter.split_markdown(text);

    assert_eq!(docs.len(), 3);
    assert_eq!(docs[0].content, "Intro text.");
    assert_eq!(docs[0].metadata.get("h1").unwrap(), "Title");

    assert_eq!(docs[1].content, "Content 1.");
    assert_eq!(docs[1].metadata.get("h1").unwrap(), "Title");
    assert_eq!(docs[1].metadata.get("h2").unwrap(), "Section 1");

    assert_eq!(docs[2].content, "Content 2.");
    assert_eq!(docs[2].metadata.get("h2").unwrap(), "Section 2");
}

#[test]
fn split_text_returns_content_only() {
    let splitter = MarkdownHeaderTextSplitter::default_headers();
    let text = "# Header\n\nParagraph 1.\n\n## Sub\n\nParagraph 2.";
    let chunks = splitter.split_text(text);

    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0], "Paragraph 1.");
    assert_eq!(chunks[1], "Paragraph 2.");
}

#[test]
fn nested_headers_clear_lower_levels() {
    let splitter = MarkdownHeaderTextSplitter::default_headers();
    let text = "# A\n\n## B\n\nText B.\n\n# C\n\nText C.";
    let docs = splitter.split_markdown(text);

    // After "# C", the h2 should be cleared
    let last = &docs[docs.len() - 1];
    assert_eq!(last.metadata.get("h1").unwrap(), "C");
    assert!(last.metadata.get("h2").is_none());
}

#[test]
fn no_content_before_first_header() {
    let splitter = MarkdownHeaderTextSplitter::default_headers();
    let text = "# Only Header\n\nSome content.";
    let docs = splitter.split_markdown(text);

    assert_eq!(docs.len(), 1);
    assert_eq!(docs[0].content, "Some content.");
}
