use synaptic_pdf::PdfLoader;

#[test]
fn constructs_with_default_mode() {
    let loader = PdfLoader::new("test.pdf");
    // Verify it compiles and accepts various path types
    let _loader2 = PdfLoader::new(std::path::PathBuf::from("/tmp/test.pdf"));
    let _ = loader;
}

#[test]
fn constructs_with_split_pages() {
    let loader = PdfLoader::with_split_pages("test.pdf");
    let _ = loader;
}

#[tokio::test]
async fn returns_error_for_missing_file() {
    use synaptic_pdf::Loader;

    let loader = PdfLoader::new("/tmp/nonexistent-synaptic-pdf-test-99999.pdf");
    let result = loader.load().await;
    assert!(result.is_err());
    let err_msg = result.unwrap_err().to_string();
    assert!(
        err_msg.contains("failed to extract text"),
        "unexpected error: {err_msg}"
    );
}

#[tokio::test]
async fn returns_error_for_corrupt_pdf() {
    use synaptic_pdf::Loader;
    use std::fs;

    // Write some garbage bytes that are not a valid PDF
    let path = std::env::temp_dir().join(format!(
        "synaptic-pdf-corrupt-{}.pdf",
        std::process::id()
    ));
    fs::write(&path, b"this is not a pdf file").unwrap();

    let loader = PdfLoader::new(&path);
    let result = loader.load().await;
    assert!(result.is_err());

    // Clean up
    let _ = fs::remove_file(&path);
}

/// This test requires a real PDF file at the specified path.
/// Run with: cargo test -p synaptic-pdf -- --ignored load_real_pdf
#[tokio::test]
#[ignore]
async fn load_real_pdf_single_document() {
    use synaptic_pdf::Loader;

    // Place a test PDF at this path to run this test
    let path = std::env::temp_dir().join("synaptic-test-sample.pdf");
    if !path.exists() {
        eprintln!(
            "Skipping: place a PDF at {} to run this test",
            path.display()
        );
        return;
    }

    let loader = PdfLoader::new(&path);
    let docs = loader.load().await.unwrap();

    assert_eq!(docs.len(), 1);
    assert!(!docs[0].content.is_empty());
    assert_eq!(
        docs[0].metadata.get("source").unwrap(),
        &serde_json::json!(path.to_string_lossy().to_string())
    );
    assert!(docs[0].metadata.contains_key("total_pages"));
}

/// This test requires a real multi-page PDF file at the specified path.
/// Run with: cargo test -p synaptic-pdf -- --ignored load_real_pdf_split
#[tokio::test]
#[ignore]
async fn load_real_pdf_split_pages() {
    use synaptic_pdf::Loader;

    // Place a multi-page test PDF at this path to run this test
    let path = std::env::temp_dir().join("synaptic-test-sample.pdf");
    if !path.exists() {
        eprintln!(
            "Skipping: place a PDF at {} to run this test",
            path.display()
        );
        return;
    }

    let loader = PdfLoader::with_split_pages(&path);
    let docs = loader.load().await.unwrap();

    assert!(!docs.is_empty());
    for (i, doc) in docs.iter().enumerate() {
        let page_num = i + 1;
        assert!(
            doc.id.ends_with(&format!(":page_{page_num}")),
            "unexpected id: {}",
            doc.id
        );
        assert!(doc.metadata.contains_key("page"));
        assert!(doc.metadata.contains_key("total_pages"));
        assert!(doc.metadata.contains_key("source"));
    }
}
