use synapse_splitters::{HtmlHeaderTextSplitter, TextSplitter};

#[test]
fn splits_on_h1_h2_h3() {
    let html = r#"<h1>Main Title</h1>
<p>Introduction paragraph.</p>
<h2>Section One</h2>
<p>Content of section one.</p>
<h3>Subsection A</h3>
<p>Details about subsection A.</p>
<h2>Section Two</h2>
<p>Content of section two.</p>"#;

    let splitter = HtmlHeaderTextSplitter::default_headers();
    let docs = splitter.split_html(html);

    assert!(
        docs.len() >= 3,
        "expected at least 3 chunks, got {}",
        docs.len()
    );

    // First chunk should have h1 metadata
    assert!(docs[0].metadata.contains_key("Header 1"));
}

#[test]
fn header_metadata_is_propagated() {
    let html = r#"<h1>Title</h1>
<p>Under title.</p>
<h2>Sub</h2>
<p>Under sub.</p>"#;

    let splitter = HtmlHeaderTextSplitter::default_headers();
    let docs = splitter.split_html(html);

    // The second chunk should have both h1 and h2 metadata
    let last = docs.last().unwrap();
    assert!(
        last.metadata.contains_key("Header 1"),
        "should have Header 1"
    );
    assert!(
        last.metadata.contains_key("Header 2"),
        "should have Header 2"
    );
}

#[test]
fn split_text_returns_content_strings() {
    let html = r#"<h1>A</h1>
<p>Content A</p>
<h1>B</h1>
<p>Content B</p>"#;

    let splitter = HtmlHeaderTextSplitter::default_headers();
    let chunks = splitter.split_text(html);

    assert_eq!(chunks.len(), 2);
    assert_eq!(chunks[0], "Content A");
    assert_eq!(chunks[1], "Content B");
}

#[test]
fn custom_headers() {
    let html = r#"<h2>Only H2</h2>
<p>Some content.</p>"#;

    let splitter = HtmlHeaderTextSplitter::new(vec![("h2".to_string(), "Section".to_string())]);
    let docs = splitter.split_html(html);

    assert_eq!(docs.len(), 1);
    assert!(docs[0].metadata.contains_key("Section"));
}

#[test]
fn empty_html() {
    let splitter = HtmlHeaderTextSplitter::default_headers();
    let docs = splitter.split_html("");
    assert!(docs.is_empty());
}
