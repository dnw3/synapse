mod character;
mod html_header;
pub mod language;
mod markdown;
mod recursive;
mod token;

pub use character::CharacterTextSplitter;
pub use html_header::HtmlHeaderTextSplitter;
pub use language::Language;
pub use markdown::{HeaderType, MarkdownHeaderTextSplitter};
pub use recursive::RecursiveCharacterTextSplitter;
pub use token::TokenTextSplitter;

// Re-export Document from core for backward compatibility
pub use synaptic_core::Document;

/// Trait for splitting text into chunks.
pub trait TextSplitter: Send + Sync {
    /// Split a string into chunks.
    fn split_text(&self, text: &str) -> Vec<String>;

    /// Split documents by splitting each document's content and producing
    /// new documents for each chunk. Metadata is preserved on each chunk.
    fn split_documents(&self, docs: Vec<Document>) -> Vec<Document> {
        let mut result = Vec::new();
        for doc in docs {
            let chunks = self.split_text(&doc.content);
            for (i, chunk) in chunks.into_iter().enumerate() {
                let mut metadata = doc.metadata.clone();
                metadata.insert(
                    "chunk_index".to_string(),
                    serde_json::Value::Number(i.into()),
                );
                result.push(Document::with_metadata(
                    format!("{}-chunk-{i}", doc.id),
                    chunk,
                    metadata,
                ));
            }
        }
        result
    }
}
