use std::collections::HashMap;
use std::path::{Path, PathBuf};

use async_trait::async_trait;
use serde_json::Value;
use synaptic_core::SynapticError;
use synaptic_retrieval::Document;

use crate::Loader;

/// Loads documents from files in a directory.
///
/// By default, only reads files in the top-level directory.
/// Use `with_recursive(true)` to include subdirectories.
/// Use `with_glob(pattern)` to filter by file extension (e.g., "*.txt").
pub struct DirectoryLoader {
    path: PathBuf,
    glob_pattern: Option<String>,
    recursive: bool,
}

impl DirectoryLoader {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self {
            path: path.into(),
            glob_pattern: None,
            recursive: false,
        }
    }

    pub fn with_glob(mut self, pattern: impl Into<String>) -> Self {
        self.glob_pattern = Some(pattern.into());
        self
    }

    pub fn with_recursive(mut self, recursive: bool) -> Self {
        self.recursive = recursive;
        self
    }

    fn collect_files(&self, dir: &Path) -> Result<Vec<PathBuf>, SynapticError> {
        let mut files = Vec::new();
        let entries = std::fs::read_dir(dir).map_err(|e| {
            SynapticError::Loader(format!("cannot read directory {}: {e}", dir.display()))
        })?;

        for entry in entries {
            let entry =
                entry.map_err(|e| SynapticError::Loader(format!("directory entry error: {e}")))?;
            let path = entry.path();

            if path.is_dir() && self.recursive {
                files.extend(self.collect_files(&path)?);
            } else if path.is_file() {
                if let Some(pattern) = &self.glob_pattern {
                    if let Some(ext_pattern) = pattern.strip_prefix("*.") {
                        if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                            if ext == ext_pattern {
                                files.push(path);
                            }
                        }
                    } else {
                        files.push(path);
                    }
                } else {
                    files.push(path);
                }
            }
        }

        files.sort();
        Ok(files)
    }
}

#[async_trait]
impl Loader for DirectoryLoader {
    async fn load(&self) -> Result<Vec<Document>, SynapticError> {
        let files = self.collect_files(&self.path)?;
        let mut docs = Vec::new();

        for file_path in files {
            let content = std::fs::read_to_string(&file_path).map_err(|e| {
                SynapticError::Loader(format!("cannot read {}: {e}", file_path.display()))
            })?;

            let id = file_path
                .strip_prefix(&self.path)
                .unwrap_or(&file_path)
                .to_string_lossy()
                .to_string();

            let mut metadata = HashMap::new();
            metadata.insert(
                "source".to_string(),
                Value::String(file_path.to_string_lossy().to_string()),
            );

            docs.push(Document::with_metadata(id, content, metadata));
        }

        Ok(docs)
    }
}
