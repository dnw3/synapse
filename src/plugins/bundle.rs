//! Bundle loader — loads Claude/Codex/Cursor static content packs.
//!
//! Bundles provide skills dirs and agent dirs only — NO runtime tools, hooks,
//! memory, or services. They are purely static content packs.

use std::path::{Path, PathBuf};

use super::discovery::PluginFormat;

/// Content extracted from a bundle.
#[derive(Debug, Default)]
pub struct BundleContent {
    pub skills_dirs: Vec<PathBuf>,
    pub agent_dirs: Vec<PathBuf>,
    pub id: String,
    pub description: String,
}

/// Load a bundle from a directory. Returns None if no content found.
pub fn load_bundle(dir: &Path, format: &PluginFormat) -> Option<BundleContent> {
    let mut content = BundleContent {
        // Default ID from directory name
        id: dir
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown")
            .to_string(),
        description: String::new(),
        skills_dirs: Vec::new(),
        agent_dirs: Vec::new(),
    };

    // Check for skills/ dir → add to skills_dirs
    let skills_dir = dir.join("skills");
    if skills_dir.is_dir() {
        content.skills_dirs.push(skills_dir);
    }

    // Check for commands/ dir → add to skills_dirs (legacy Claude)
    let commands_dir = dir.join("commands");
    if commands_dir.is_dir() {
        content.skills_dirs.push(commands_dir);
    }

    // Check for agents/ dir → add to agent_dirs
    let agents_dir = dir.join("agents");
    if agents_dir.is_dir() {
        content.agent_dirs.push(agents_dir);
    }

    // Read manifest JSON if present, based on format
    let manifest_path = manifest_path_for_format(dir, format);
    if let Some(path) = manifest_path {
        if let Ok(data) = std::fs::read_to_string(&path) {
            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&data) {
                if let Some(id) = json.get("id").and_then(|v| v.as_str()) {
                    content.id = id.to_string();
                }
                if let Some(desc) = json.get("description").and_then(|v| v.as_str()) {
                    content.description = desc.to_string();
                }
            }
        }
    }

    // Return None if no content found
    if content.skills_dirs.is_empty() && content.agent_dirs.is_empty() {
        return None;
    }

    Some(content)
}

/// Return the manifest path for the given plugin format, if applicable.
fn manifest_path_for_format(dir: &Path, format: &PluginFormat) -> Option<PathBuf> {
    match format {
        PluginFormat::ClaudeBundle => {
            let p = dir.join(".claude-plugin").join("plugin.json");
            if p.is_file() {
                Some(p)
            } else {
                None
            }
        }
        PluginFormat::CodexBundle => {
            let p = dir.join(".codex-plugin").join("plugin.json");
            if p.is_file() {
                Some(p)
            } else {
                None
            }
        }
        PluginFormat::CursorBundle => {
            let p = dir.join(".cursor-plugin").join("plugin.json");
            if p.is_file() {
                Some(p)
            } else {
                None
            }
        }
        // SynapseNative and OpenClaw use different manifest formats; no JSON bundle manifest
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn load_bundle_with_skills() {
        let root = TempDir::new().unwrap();
        let dir = root.path();
        fs::create_dir_all(dir.join("skills")).unwrap();

        let result = load_bundle(dir, &PluginFormat::ClaudeBundle);
        assert!(result.is_some(), "Expected Some but got None");
        let content = result.unwrap();
        assert_eq!(content.skills_dirs.len(), 1);
        assert!(content.skills_dirs[0].ends_with("skills"));
        assert!(content.agent_dirs.is_empty());
    }

    #[test]
    fn load_bundle_with_agents() {
        let root = TempDir::new().unwrap();
        let dir = root.path();
        fs::create_dir_all(dir.join("agents")).unwrap();

        let result = load_bundle(dir, &PluginFormat::ClaudeBundle);
        assert!(result.is_some(), "Expected Some but got None");
        let content = result.unwrap();
        assert_eq!(content.agent_dirs.len(), 1);
        assert!(content.agent_dirs[0].ends_with("agents"));
        assert!(content.skills_dirs.is_empty());
    }

    #[test]
    fn load_bundle_empty_returns_none() {
        let root = TempDir::new().unwrap();
        let dir = root.path();

        let result = load_bundle(dir, &PluginFormat::ClaudeBundle);
        assert!(result.is_none(), "Expected None for empty dir but got Some");
    }

    #[test]
    fn load_bundle_reads_manifest() {
        let root = TempDir::new().unwrap();
        let dir = root.path();

        // Create skills dir so bundle is non-empty
        fs::create_dir_all(dir.join("skills")).unwrap();

        // Create .claude-plugin/plugin.json with id and description
        let manifest_dir = dir.join(".claude-plugin");
        fs::create_dir_all(&manifest_dir).unwrap();
        fs::write(
            manifest_dir.join("plugin.json"),
            r#"{"id": "my-bundle", "description": "A test bundle"}"#,
        )
        .unwrap();

        let result = load_bundle(dir, &PluginFormat::ClaudeBundle);
        assert!(result.is_some());
        let content = result.unwrap();
        assert_eq!(content.id, "my-bundle");
        assert_eq!(content.description, "A test bundle");
    }
}
