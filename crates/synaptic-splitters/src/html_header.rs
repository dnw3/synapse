use std::collections::HashMap;

use serde_json::Value;
use synaptic_retrieval::Document;

use crate::TextSplitter;

/// Splits HTML content by header tags (h1, h2, h3, etc.).
///
/// Each section between headers becomes a separate document/chunk.
/// Header text is added as metadata on the resulting chunks.
pub struct HtmlHeaderTextSplitter {
    /// Pairs of (tag, metadata_key) to split on.
    headers_to_split_on: Vec<(String, String)>,
}

impl HtmlHeaderTextSplitter {
    /// Create a new splitter with custom header-to-metadata-key mappings.
    ///
    /// Each tuple is `(tag_name, metadata_key)`, e.g., `("h1", "Header 1")`.
    pub fn new(headers_to_split_on: Vec<(String, String)>) -> Self {
        Self {
            headers_to_split_on,
        }
    }

    /// Default configuration: split on h1, h2, h3.
    pub fn default_headers() -> Self {
        Self::new(vec![
            ("h1".to_string(), "Header 1".to_string()),
            ("h2".to_string(), "Header 2".to_string()),
            ("h3".to_string(), "Header 3".to_string()),
        ])
    }

    /// Split HTML and return documents with header metadata.
    pub fn split_html(&self, text: &str) -> Vec<Document> {
        let mut documents = Vec::new();
        let mut current_headers: HashMap<String, String> = HashMap::new();
        let mut current_content = String::new();
        let mut doc_index = 0;

        // Build a sorted list of header tags by priority (h1 < h2 < h3, etc.)
        // so we can clear lower-level headers when a higher-level one appears.
        let header_levels: HashMap<String, usize> = self
            .headers_to_split_on
            .iter()
            .enumerate()
            .map(|(i, (tag, _))| (tag.to_lowercase(), i))
            .collect();

        for line in text.lines() {
            let trimmed = line.trim();

            // Try to match an opening header tag
            let mut matched = None;
            for (tag, metadata_key) in &self.headers_to_split_on {
                let open_tag = format!("<{}", tag.to_lowercase());
                let trimmed_lower = trimmed.to_lowercase();

                if trimmed_lower.starts_with(&open_tag) {
                    // Extract text between opening and closing tags
                    let header_text = extract_tag_content(trimmed, tag);
                    matched = Some((tag.clone(), metadata_key.clone(), header_text));
                    break;
                }
            }

            if let Some((tag, metadata_key, header_text)) = matched {
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

                // Clear headers of same or lower level (higher index)
                let current_level = header_levels.get(&tag.to_lowercase()).copied().unwrap_or(0);
                let keys_to_remove: Vec<String> = current_headers
                    .keys()
                    .filter(|k| {
                        self.headers_to_split_on
                            .iter()
                            .find(|(_, mk)| mk == *k)
                            .and_then(|(t, _)| header_levels.get(&t.to_lowercase()))
                            .map(|level| *level >= current_level)
                            .unwrap_or(false)
                    })
                    .cloned()
                    .collect();
                for key in keys_to_remove {
                    current_headers.remove(&key);
                }

                current_headers.insert(metadata_key, header_text);
                current_content.clear();
            } else {
                // Strip simple HTML tags for content
                let stripped = strip_simple_tags(trimmed);
                let stripped = stripped.trim();
                if !stripped.is_empty() {
                    if !current_content.is_empty() {
                        current_content.push('\n');
                    }
                    current_content.push_str(stripped);
                }
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

/// Extract text content between an opening and closing HTML tag.
fn extract_tag_content(line: &str, tag: &str) -> String {
    let close_tag = format!("</{}>", tag.to_lowercase());
    // Find the end of the opening tag (after ">")
    if let Some(start) = line.find('>') {
        let rest = &line[start + 1..];
        // Find closing tag
        let lower_rest = rest.to_lowercase();
        if let Some(end) = lower_rest.find(&close_tag) {
            return rest[..end].trim().to_string();
        }
        // No closing tag found, return everything after opening tag
        return rest.trim().to_string();
    }
    String::new()
}

/// Strip simple HTML tags from a string (basic implementation).
fn strip_simple_tags(text: &str) -> String {
    let mut result = String::new();
    let mut in_tag = false;
    for ch in text.chars() {
        if ch == '<' {
            in_tag = true;
        } else if ch == '>' {
            in_tag = false;
        } else if !in_tag {
            result.push(ch);
        }
    }
    result
}

impl TextSplitter for HtmlHeaderTextSplitter {
    fn split_text(&self, text: &str) -> Vec<String> {
        self.split_html(text)
            .into_iter()
            .map(|d| d.content)
            .collect()
    }
}
