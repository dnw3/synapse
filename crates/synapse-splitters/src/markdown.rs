use std::collections::HashMap;

use serde_json::Value;
use synapse_retrieval::Document;

use crate::TextSplitter;

/// A markdown header level and its text.
#[derive(Debug, Clone)]
pub struct HeaderType {
    /// The markdown header prefix (e.g., "#", "##", "###")
    pub level: String,
    /// The metadata key to store this header's text under
    pub name: String,
}

/// Splits markdown text by headers, adding header hierarchy to metadata.
///
/// Each section between headers becomes a separate document.
pub struct MarkdownHeaderTextSplitter {
    headers_to_split_on: Vec<HeaderType>,
}

impl MarkdownHeaderTextSplitter {
    pub fn new(headers_to_split_on: Vec<HeaderType>) -> Self {
        Self {
            headers_to_split_on,
        }
    }

    /// Default configuration: split on #, ##, ###.
    pub fn default_headers() -> Self {
        Self::new(vec![
            HeaderType {
                level: "#".to_string(),
                name: "h1".to_string(),
            },
            HeaderType {
                level: "##".to_string(),
                name: "h2".to_string(),
            },
            HeaderType {
                level: "###".to_string(),
                name: "h3".to_string(),
            },
        ])
    }

    /// Split markdown and return documents with header metadata.
    pub fn split_markdown(&self, text: &str) -> Vec<Document> {
        let mut documents = Vec::new();
        let mut current_headers: HashMap<String, String> = HashMap::new();
        let mut current_content = String::new();
        let mut doc_index = 0;

        for line in text.lines() {
            let trimmed = line.trim();

            // Check if this line is a header we should split on
            let mut matched_header = None;
            for header_type in &self.headers_to_split_on {
                let prefix = format!("{} ", header_type.level);
                if trimmed.starts_with(&prefix) {
                    matched_header =
                        Some((header_type, trimmed[prefix.len()..].trim().to_string()));
                    break;
                }
            }

            if let Some((header_type, header_text)) = matched_header {
                // Save current content as a document if non-empty
                let content = current_content.trim().to_string();
                if !content.is_empty() {
                    let mut metadata: HashMap<String, Value> = current_headers
                        .iter()
                        .map(|(k, v)| (k.clone(), Value::String(v.clone())))
                        .collect();
                    metadata.insert("chunk_index".to_string(), Value::Number(doc_index.into()));
                    documents.push(Document::with_metadata(
                        format!("chunk-{doc_index}"),
                        content,
                        metadata,
                    ));
                    doc_index += 1;
                }

                // Clear headers of same or lower level
                let current_level = header_type.level.len();
                let keys_to_remove: Vec<String> = current_headers
                    .keys()
                    .filter(|k| {
                        self.headers_to_split_on
                            .iter()
                            .find(|h| h.name == **k)
                            .map(|h| h.level.len() >= current_level)
                            .unwrap_or(false)
                    })
                    .cloned()
                    .collect();
                for key in keys_to_remove {
                    current_headers.remove(&key);
                }

                current_headers.insert(header_type.name.clone(), header_text);
                current_content.clear();
            } else {
                if !current_content.is_empty() {
                    current_content.push('\n');
                }
                current_content.push_str(line);
            }
        }

        // Don't forget the last section
        let content = current_content.trim().to_string();
        if !content.is_empty() {
            let mut metadata: HashMap<String, Value> = current_headers
                .iter()
                .map(|(k, v)| (k.clone(), Value::String(v.clone())))
                .collect();
            metadata.insert("chunk_index".to_string(), Value::Number(doc_index.into()));
            documents.push(Document::with_metadata(
                format!("chunk-{doc_index}"),
                content,
                metadata,
            ));
        }

        documents
    }
}

impl TextSplitter for MarkdownHeaderTextSplitter {
    fn split_text(&self, text: &str) -> Vec<String> {
        self.split_markdown(text)
            .into_iter()
            .map(|d| d.content)
            .collect()
    }
}
