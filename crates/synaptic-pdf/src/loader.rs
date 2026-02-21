use std::collections::HashMap;
use std::path::PathBuf;

use async_trait::async_trait;
use serde_json::Value;
use synaptic_core::{Document, Loader, SynapticError};

/// Loads documents from a PDF file.
///
/// Uses `pdf_extract` to extract text content from PDF files. Supports two
/// modes of operation:
///
/// - **Single document** (default): All pages are combined into one `Document`.
/// - **Split pages**: Each page becomes a separate `Document`, split on form
///   feed characters (`\x0c`) that `pdf_extract` inserts between pages.
///
/// # Examples
///
/// ```no_run
/// use synaptic_pdf::{PdfLoader, Loader};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// // Load entire PDF as one document
/// let loader = PdfLoader::new("document.pdf");
/// let docs = loader.load().await?;
/// assert_eq!(docs.len(), 1);
///
/// // Load with one document per page
/// let loader = PdfLoader::with_split_pages("document.pdf");
/// let docs = loader.load().await?;
/// // docs.len() == number of pages
/// # Ok(())
/// # }
/// ```
pub struct PdfLoader {
    path: PathBuf,
    split_pages: bool,
}

impl PdfLoader {
    /// Create a new `PdfLoader` that extracts all text as a single document.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            split_pages: false,
        }
    }

    /// Create a new `PdfLoader` that splits text into one document per page.
    ///
    /// Page boundaries are detected by form feed characters (`\x0c`) inserted
    /// by the PDF extraction library.
    pub fn with_split_pages(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            split_pages: true,
        }
    }
}

#[async_trait]
impl Loader for PdfLoader {
    async fn load(&self) -> Result<Vec<Document>, SynapticError> {
        let path = self.path.clone();
        let split_pages = self.split_pages;

        // pdf_extract::extract_text is synchronous, so run it on a blocking thread
        let text = tokio::task::spawn_blocking(move || {
            pdf_extract::extract_text(&path)
        })
        .await
        .map_err(|e| SynapticError::Loader(format!("task join error: {e}")))?
        .map_err(|e| {
            SynapticError::Loader(format!(
                "failed to extract text from {}: {e}",
                self.path.display()
            ))
        })?;

        let path_str = self.path.to_string_lossy().to_string();

        if split_pages {
            // Split on form feed characters that pdf_extract inserts between pages
            let pages: Vec<&str> = text.split('\x0c').collect();
            let total_pages = pages.len();

            let docs = pages
                .into_iter()
                .enumerate()
                .filter(|(_, content)| !content.trim().is_empty())
                .map(|(i, content)| {
                    let page_num = i + 1;
                    let id = format!("{path_str}:page_{page_num}");

                    let mut metadata = HashMap::new();
                    metadata.insert("source".to_string(), Value::String(path_str.clone()));
                    metadata.insert(
                        "page".to_string(),
                        Value::Number(serde_json::Number::from(page_num)),
                    );
                    metadata.insert(
                        "total_pages".to_string(),
                        Value::Number(serde_json::Number::from(total_pages)),
                    );

                    Document::with_metadata(id, content.trim(), metadata)
                })
                .collect();

            Ok(docs)
        } else {
            // Count pages from form feed characters
            let total_pages = text.split('\x0c').count();

            let mut metadata = HashMap::new();
            metadata.insert("source".to_string(), Value::String(path_str.clone()));
            metadata.insert(
                "total_pages".to_string(),
                Value::Number(serde_json::Number::from(total_pages)),
            );

            Ok(vec![Document::with_metadata(path_str, text.trim(), metadata)])
        }
    }
}
