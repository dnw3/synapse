use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::Value;
use tokio::fs;

use synaptic_core::{Embeddings, Item, Store, SynapticError};

fn now_iso() -> String {
    format!("{:?}", std::time::SystemTime::now())
}

/// File-system backed implementation of `Store`.
///
/// Layout: `{root}/{namespace_path}/{key}.json`
/// where namespace_path joins namespace segments with `/`.
pub struct FileStore {
    root: PathBuf,
    embeddings: Option<Arc<dyn Embeddings>>,
}

impl FileStore {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: root.into(),
            embeddings: None,
        }
    }

    pub fn with_embeddings(mut self, embeddings: Arc<dyn Embeddings>) -> Self {
        self.embeddings = Some(embeddings);
        self
    }

    fn namespace_dir(&self, namespace: &[&str]) -> PathBuf {
        let mut path = self.root.clone();
        for part in namespace {
            path.push(part);
        }
        path
    }

    fn item_path(&self, namespace: &[&str], key: &str) -> PathBuf {
        self.namespace_dir(namespace).join(format!("{}.json", key))
    }
}

#[async_trait]
impl Store for FileStore {
    async fn get(&self, namespace: &[&str], key: &str) -> Result<Option<Item>, SynapticError> {
        let path = self.item_path(namespace, key);
        match fs::read_to_string(&path).await {
            Ok(content) => {
                let item: Item = serde_json::from_str(&content).map_err(|e| {
                    SynapticError::Store(format!("failed to parse {}: {}", path.display(), e))
                })?;
                Ok(Some(item))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
            Err(e) => Err(SynapticError::Store(format!(
                "failed to read {}: {}",
                path.display(),
                e
            ))),
        }
    }

    async fn search(
        &self,
        namespace: &[&str],
        query: Option<&str>,
        limit: usize,
    ) -> Result<Vec<Item>, SynapticError> {
        let dir = self.namespace_dir(namespace);
        if !dir.exists() {
            return Ok(vec![]);
        }

        let mut items = Vec::new();
        let mut entries = fs::read_dir(&dir).await.map_err(|e| {
            SynapticError::Store(format!("failed to read dir {}: {}", dir.display(), e))
        })?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| SynapticError::Store(format!("failed to read entry: {}", e)))?
        {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") {
                continue;
            }
            // Skip subdirectories
            if path.is_dir() {
                continue;
            }

            let content = fs::read_to_string(&path).await.map_err(|e| {
                SynapticError::Store(format!("failed to read {}: {}", path.display(), e))
            })?;

            let item: Item = match serde_json::from_str(&content) {
                Ok(item) => item,
                Err(_) => continue, // Skip malformed files
            };

            // Apply substring filter
            if let Some(q) = query {
                if !item.key.contains(q) && !item.value.to_string().contains(q) {
                    continue;
                }
            }

            items.push(item);
            if items.len() >= limit {
                break;
            }
        }

        Ok(items)
    }

    async fn put(&self, namespace: &[&str], key: &str, value: Value) -> Result<(), SynapticError> {
        let dir = self.namespace_dir(namespace);
        fs::create_dir_all(&dir).await.map_err(|e| {
            SynapticError::Store(format!("failed to create dir {}: {}", dir.display(), e))
        })?;

        let path = self.item_path(namespace, key);
        let now = now_iso();

        // Check if existing item to preserve created_at
        let created_at = match fs::read_to_string(&path).await {
            Ok(content) => serde_json::from_str::<Item>(&content)
                .map(|item| item.created_at)
                .unwrap_or_else(|_| now.clone()),
            Err(_) => now.clone(),
        };

        let item = Item {
            namespace: namespace.iter().map(|s| s.to_string()).collect(),
            key: key.to_string(),
            value,
            created_at,
            updated_at: now,
            score: None,
        };

        let json = serde_json::to_string_pretty(&item)
            .map_err(|e| SynapticError::Store(format!("failed to serialize: {}", e)))?;

        fs::write(&path, json).await.map_err(|e| {
            SynapticError::Store(format!("failed to write {}: {}", path.display(), e))
        })?;

        Ok(())
    }

    async fn delete(&self, namespace: &[&str], key: &str) -> Result<(), SynapticError> {
        let path = self.item_path(namespace, key);
        match fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(SynapticError::Store(format!(
                "failed to delete {}: {}",
                path.display(),
                e
            ))),
        }
    }

    async fn list_namespaces(&self, prefix: &[&str]) -> Result<Vec<Vec<String>>, SynapticError> {
        let base = if prefix.is_empty() {
            self.root.clone()
        } else {
            self.namespace_dir(prefix)
        };

        if !base.exists() {
            return Ok(vec![]);
        }

        let mut namespaces = Vec::new();
        collect_namespaces(&base, &self.root, &mut namespaces).await?;
        Ok(namespaces)
    }
}

/// Recursively collect namespace paths that contain .json files.
async fn collect_namespaces(
    dir: &Path,
    root: &Path,
    result: &mut Vec<Vec<String>>,
) -> Result<(), SynapticError> {
    let mut entries = fs::read_dir(dir).await.map_err(|e| {
        SynapticError::Store(format!("failed to read dir {}: {}", dir.display(), e))
    })?;

    let mut has_json = false;
    let mut subdirs = Vec::new();

    while let Some(entry) = entries
        .next_entry()
        .await
        .map_err(|e| SynapticError::Store(format!("failed to read entry: {}", e)))?
    {
        let path = entry.path();
        if path.is_dir() {
            subdirs.push(path);
        } else if path.extension().and_then(|e| e.to_str()) == Some("json") {
            has_json = true;
        }
    }

    if has_json {
        // Convert dir path relative to root into namespace segments
        let rel = dir.strip_prefix(root).unwrap_or(dir);
        let ns: Vec<String> = rel
            .components()
            .filter_map(|c| c.as_os_str().to_str().map(String::from))
            .collect();
        if !ns.is_empty() {
            result.push(ns);
        }
    }

    for subdir in subdirs {
        Box::pin(collect_namespaces(&subdir, root, result)).await?;
    }

    Ok(())
}
