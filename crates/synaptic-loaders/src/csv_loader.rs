use std::collections::HashMap;

use async_trait::async_trait;
use serde_json::Value;
use synaptic_core::SynapticError;
use synaptic_retrieval::Document;

use crate::Loader;

/// Loads documents from CSV data. Each row becomes a Document.
///
/// The `content_column` specifies which column to use as document content.
/// If not set, all columns are concatenated.
/// An `id_column` can optionally specify the column for document IDs.
pub struct CsvLoader {
    data: String,
    content_column: Option<String>,
    id_column: Option<String>,
}

impl CsvLoader {
    pub fn new(data: impl Into<String>) -> Self {
        Self {
            data: data.into(),
            content_column: None,
            id_column: None,
        }
    }

    pub fn with_content_column(mut self, column: impl Into<String>) -> Self {
        self.content_column = Some(column.into());
        self
    }

    pub fn with_id_column(mut self, column: impl Into<String>) -> Self {
        self.id_column = Some(column.into());
        self
    }
}

#[async_trait]
impl Loader for CsvLoader {
    async fn load(&self) -> Result<Vec<Document>, SynapticError> {
        let mut reader = csv::Reader::from_reader(self.data.as_bytes());
        let headers = reader
            .headers()
            .map_err(|e| SynapticError::Loader(format!("CSV header error: {e}")))?
            .clone();

        let mut docs = Vec::new();

        for (i, result) in reader.records().enumerate() {
            let record =
                result.map_err(|e| SynapticError::Loader(format!("CSV row {i} error: {e}")))?;

            let id = if let Some(id_col) = &self.id_column {
                let idx = headers
                    .iter()
                    .position(|h| h == id_col.as_str())
                    .ok_or_else(|| {
                        SynapticError::Loader(format!("id column '{id_col}' not found"))
                    })?;
                record.get(idx).unwrap_or("").to_string()
            } else {
                format!("row-{i}")
            };

            let content = if let Some(content_col) = &self.content_column {
                let idx = headers
                    .iter()
                    .position(|h| h == content_col.as_str())
                    .ok_or_else(|| {
                        SynapticError::Loader(format!("content column '{content_col}' not found"))
                    })?;
                record.get(idx).unwrap_or("").to_string()
            } else {
                // Concatenate all columns
                record.iter().collect::<Vec<&str>>().join(" ")
            };

            // Store all columns as metadata
            let mut metadata = HashMap::new();
            for (j, header) in headers.iter().enumerate() {
                if let Some(value) = record.get(j) {
                    metadata.insert(header.to_string(), Value::String(value.to_string()));
                }
            }

            docs.push(Document::with_metadata(id, content, metadata));
        }

        Ok(docs)
    }
}
