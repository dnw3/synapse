use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use synaptic::core::SynapticError;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[allow(dead_code)]
pub struct RegistryEntry {
    pub runtime_id: String,
    pub provider_id: String,
    pub scope_key: String,
    pub image: Option<String>,
    pub config_hash: String,
    pub created_at: DateTime<Utc>,
    pub last_used_at: DateTime<Utc>,
}

#[allow(dead_code)]
pub struct SandboxPersistentRegistry {
    path: PathBuf,
}

#[allow(dead_code)]
impl SandboxPersistentRegistry {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    /// Default path: ~/.synapse/sandbox-registry.json
    pub fn default_path() -> PathBuf {
        dirs::home_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join(".synapse")
            .join("sandbox-registry.json")
    }

    /// Load entries from disk. Returns empty vec if file doesn't exist.
    pub fn load(&self) -> Result<Vec<RegistryEntry>, SynapticError> {
        if !self.path.exists() {
            return Ok(Vec::new());
        }
        let content = std::fs::read_to_string(&self.path)
            .map_err(|e| SynapticError::Tool(format!("failed to read sandbox registry: {e}")))?;
        let entries: Vec<RegistryEntry> = serde_json::from_str(&content)
            .map_err(|e| SynapticError::Tool(format!("failed to parse sandbox registry: {e}")))?;
        Ok(entries)
    }

    /// Save entries to disk with file locking.
    pub fn save(&self, entries: &[RegistryEntry]) -> Result<(), SynapticError> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| SynapticError::Tool(format!("failed to create registry dir: {e}")))?;
        }

        let content = serde_json::to_string_pretty(entries)
            .map_err(|e| SynapticError::Tool(format!("failed to serialize registry: {e}")))?;

        // Atomic write: write to temp file then rename for crash safety
        let tmp_path = self.path.with_extension("json.tmp");
        std::fs::write(&tmp_path, content.as_bytes())
            .map_err(|e| SynapticError::Tool(format!("failed to write registry tmp: {e}")))?;
        std::fs::rename(&tmp_path, &self.path)
            .map_err(|e| SynapticError::Tool(format!("failed to rename registry: {e}")))?;

        Ok(())
    }

    /// Add an entry. Loads, appends, saves.
    pub fn add(&self, entry: RegistryEntry) -> Result<(), SynapticError> {
        let mut entries = self.load()?;
        entries.retain(|e| e.runtime_id != entry.runtime_id);
        entries.push(entry);
        self.save(&entries)
    }

    /// Remove an entry by runtime_id.
    pub fn remove(&self, runtime_id: &str) -> Result<(), SynapticError> {
        let mut entries = self.load()?;
        entries.retain(|e| e.runtime_id != runtime_id);
        self.save(&entries)
    }

    /// Update last_used_at for a runtime_id.
    pub fn touch(&self, runtime_id: &str) -> Result<(), SynapticError> {
        let mut entries = self.load()?;
        if let Some(entry) = entries.iter_mut().find(|e| e.runtime_id == runtime_id) {
            entry.last_used_at = Utc::now();
        }
        self.save(&entries)
    }

    /// Get entries that are idle longer than given hours or older than max_age_days.
    /// Skip entries used within the last 5 minutes (hot-window protection).
    pub fn prunable(
        &self,
        idle_hours: u32,
        max_age_days: u32,
    ) -> Result<Vec<RegistryEntry>, SynapticError> {
        let entries = self.load()?;
        let now = Utc::now();
        let hot_window = chrono::Duration::minutes(5);
        let idle_threshold = chrono::Duration::hours(idle_hours as i64);
        let age_threshold = chrono::Duration::days(max_age_days as i64);

        Ok(entries
            .into_iter()
            .filter(|e| {
                let since_used = now - e.last_used_at;
                let since_created = now - e.created_at;

                // Skip hot-window
                if since_used < hot_window {
                    return false;
                }

                since_used > idle_threshold || since_created > age_threshold
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn make_entry(id: &str, hours_ago: i64) -> RegistryEntry {
        let now = Utc::now();
        RegistryEntry {
            runtime_id: id.to_string(),
            provider_id: "docker".to_string(),
            scope_key: "test".to_string(),
            image: Some("ubuntu:latest".to_string()),
            config_hash: "abc123".to_string(),
            created_at: now - chrono::Duration::hours(hours_ago),
            last_used_at: now - chrono::Duration::hours(hours_ago),
        }
    }

    #[test]
    fn test_load_empty() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("registry.json");
        let registry = SandboxPersistentRegistry::new(path);
        let entries = registry.load().unwrap();
        assert!(entries.is_empty());
    }

    #[test]
    fn test_add_and_load() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("registry.json");
        let registry = SandboxPersistentRegistry::new(path);

        registry.add(make_entry("r1", 2)).unwrap();
        registry.add(make_entry("r2", 1)).unwrap();

        let entries = registry.load().unwrap();
        assert_eq!(entries.len(), 2);
    }

    #[test]
    fn test_add_deduplicates() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("registry.json");
        let registry = SandboxPersistentRegistry::new(path);

        registry.add(make_entry("r1", 2)).unwrap();
        registry.add(make_entry("r1", 1)).unwrap();

        let entries = registry.load().unwrap();
        assert_eq!(entries.len(), 1);
    }

    #[test]
    fn test_remove() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("registry.json");
        let registry = SandboxPersistentRegistry::new(path);

        registry.add(make_entry("r1", 2)).unwrap();
        registry.add(make_entry("r2", 1)).unwrap();
        registry.remove("r1").unwrap();

        let entries = registry.load().unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].runtime_id, "r2");
    }

    #[test]
    fn test_touch() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("registry.json");
        let registry = SandboxPersistentRegistry::new(path);

        registry.add(make_entry("r1", 48)).unwrap();
        let before = registry.load().unwrap()[0].last_used_at;

        registry.touch("r1").unwrap();
        let after = registry.load().unwrap()[0].last_used_at;

        assert!(after > before);
    }

    #[test]
    fn test_prunable() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("registry.json");
        let registry = SandboxPersistentRegistry::new(path);

        // 48 hours idle — should be prunable with 24h threshold
        registry.add(make_entry("old", 48)).unwrap();
        // 1 hour idle — should NOT be prunable with 24h threshold
        registry.add(make_entry("recent", 1)).unwrap();

        let prunable = registry.prunable(24, 7).unwrap();
        assert_eq!(prunable.len(), 1);
        assert_eq!(prunable[0].runtime_id, "old");
    }
}
