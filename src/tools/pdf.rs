//! PDF reading tool for the Deep Agent.
//!
//! Uses pdf-extract to extract text from PDF files (the same library
//! that synaptic-pdf's PdfLoader uses internally).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use synaptic::core::{SynapticError, Tool};

/// Tool that reads and extracts text from PDF files.
pub struct ReadPdfTool {
    work_dir: PathBuf,
}

impl ReadPdfTool {
    pub fn new(work_dir: &Path) -> Arc<dyn Tool> {
        Arc::new(Self {
            work_dir: work_dir.to_path_buf(),
        })
    }
}

#[async_trait]
impl Tool for ReadPdfTool {
    fn name(&self) -> &'static str {
        "read_pdf"
    }

    fn description(&self) -> &'static str {
        "Read and extract text content from a PDF file. Returns the full text of the PDF."
    }

    fn parameters(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "path": {
                    "type": "string",
                    "description": "Path to the PDF file (relative to working directory or absolute)."
                },
                "page": {
                    "type": "integer",
                    "description": "Optional: specific page number to extract (1-indexed). If omitted, extracts all pages."
                }
            },
            "required": ["path"]
        }))
    }

    async fn call(&self, args: Value) -> Result<Value, SynapticError> {
        let path_str = args
            .get("path")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SynapticError::Tool("missing 'path' argument".into()))?;

        let full_path = if Path::new(path_str).is_absolute() {
            PathBuf::from(path_str)
        } else {
            self.work_dir.join(path_str)
        };

        if !full_path.exists() {
            return Err(SynapticError::Tool(format!(
                "PDF file not found: {}",
                path_str
            )));
        }

        // Use pdf_extract directly (same as synaptic-pdf does internally)
        let path_clone = full_path.clone();
        let text = tokio::task::spawn_blocking(move || {
            pdf_extract::extract_text(&path_clone)
                .map_err(|e| SynapticError::Tool(format!("failed to extract PDF text: {}", e)))
        })
        .await
        .map_err(|e| SynapticError::Tool(format!("PDF task failed: {}", e)))??;

        // Handle optional page filter
        if let Some(page_num) = args.get("page").and_then(|v| v.as_u64()) {
            let pages: Vec<&str> = text.split('\u{c}').collect(); // form feed = page break
            let idx = (page_num as usize).saturating_sub(1);
            if idx < pages.len() {
                return Ok(json!({
                    "path": path_str,
                    "page": page_num,
                    "total_pages": pages.len(),
                    "content": pages[idx].trim(),
                }));
            } else {
                return Err(SynapticError::Tool(format!(
                    "page {} out of range (total: {})",
                    page_num,
                    pages.len()
                )));
            }
        }

        tracing::info!(path = %path_str, "PDF read");

        let page_count = text.matches('\u{c}').count() + 1;
        Ok(json!({
            "path": path_str,
            "total_pages": page_count,
            "content": text.trim(),
        }))
    }
}
