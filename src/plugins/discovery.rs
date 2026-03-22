use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct DiscoveredPlugin {
    pub path: PathBuf,
    pub format: PluginFormat,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PluginFormat {
    SynapseNative, // plugin.toml
    OpenClaw,      // openclaw.plugin.json
    ClaudeBundle,  // .claude-plugin/plugin.json or skills/ dir
    CodexBundle,   // .codex-plugin/plugin.json
    CursorBundle,  // .cursor-plugin/plugin.json
}

/// Scan a directory for plugins (each subdirectory is checked).
pub fn discover_plugins(dir: &Path) -> Vec<DiscoveredPlugin> {
    let mut plugins = Vec::new();

    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return plugins,
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        if let Some(plugin) = detect_plugin(&path) {
            plugins.push(plugin);
        }
    }

    plugins
}

/// Detect the plugin format in a single directory, applying priority order.
fn detect_plugin(dir: &Path) -> Option<DiscoveredPlugin> {
    // Priority 1: plugin.toml → SynapseNative
    if dir.join("plugin.toml").is_file() {
        return Some(DiscoveredPlugin {
            path: dir.to_path_buf(),
            format: PluginFormat::SynapseNative,
        });
    }

    // Priority 2: openclaw.plugin.json → OpenClaw
    if dir.join("openclaw.plugin.json").is_file() {
        return Some(DiscoveredPlugin {
            path: dir.to_path_buf(),
            format: PluginFormat::OpenClaw,
        });
    }

    // Priority 3: .codex-plugin/plugin.json → CodexBundle
    if dir.join(".codex-plugin").join("plugin.json").is_file() {
        return Some(DiscoveredPlugin {
            path: dir.to_path_buf(),
            format: PluginFormat::CodexBundle,
        });
    }

    // Priority 4: .cursor-plugin/plugin.json → CursorBundle
    if dir.join(".cursor-plugin").join("plugin.json").is_file() {
        return Some(DiscoveredPlugin {
            path: dir.to_path_buf(),
            format: PluginFormat::CursorBundle,
        });
    }

    // Priority 5: .claude-plugin/plugin.json → ClaudeBundle
    if dir.join(".claude-plugin").join("plugin.json").is_file() {
        return Some(DiscoveredPlugin {
            path: dir.to_path_buf(),
            format: PluginFormat::ClaudeBundle,
        });
    }

    // Priority 6: Has skills/ or commands/ dir → ClaudeBundle (manifestless)
    if dir.join("skills").is_dir() || dir.join("commands").is_dir() {
        return Some(DiscoveredPlugin {
            path: dir.to_path_buf(),
            format: PluginFormat::ClaudeBundle,
        });
    }

    None
}

/// Get default plugin discovery paths.
pub fn default_plugin_paths() -> Vec<PathBuf> {
    let mut paths = Vec::new();

    // ~/.synapse/plugins
    if let Some(home) = dirs::home_dir() {
        paths.push(home.join(".synapse").join("plugins"));
    }

    // .synapse/plugins (relative to cwd)
    paths.push(PathBuf::from(".synapse").join("plugins"));

    paths
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_plugin_dir(root: &TempDir, name: &str) -> PathBuf {
        let dir = root.path().join(name);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn detect_synapse_native() {
        let root = TempDir::new().unwrap();
        let plugin_dir = make_plugin_dir(&root, "my-plugin");
        fs::write(plugin_dir.join("plugin.toml"), "[plugin]\nname = \"test\"").unwrap();

        let found = discover_plugins(root.path());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].format, PluginFormat::SynapseNative);
        assert_eq!(found[0].path, plugin_dir);
    }

    #[test]
    fn detect_openclaw() {
        let root = TempDir::new().unwrap();
        let plugin_dir = make_plugin_dir(&root, "oc-plugin");
        fs::write(plugin_dir.join("openclaw.plugin.json"), "{}").unwrap();

        let found = discover_plugins(root.path());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].format, PluginFormat::OpenClaw);
    }

    #[test]
    fn detect_claude_bundle_skills() {
        let root = TempDir::new().unwrap();
        let plugin_dir = make_plugin_dir(&root, "claude-plugin");
        fs::create_dir_all(plugin_dir.join("skills")).unwrap();

        let found = discover_plugins(root.path());
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].format, PluginFormat::ClaudeBundle);
    }

    #[test]
    fn priority_native_over_openclaw() {
        let root = TempDir::new().unwrap();
        let plugin_dir = make_plugin_dir(&root, "mixed-plugin");
        fs::write(plugin_dir.join("plugin.toml"), "[plugin]\nname = \"test\"").unwrap();
        fs::write(plugin_dir.join("openclaw.plugin.json"), "{}").unwrap();

        let found = discover_plugins(root.path());
        assert_eq!(found.len(), 1);
        // SynapseNative must win over OpenClaw
        assert_eq!(found[0].format, PluginFormat::SynapseNative);
    }

    #[test]
    fn empty_dir_returns_nothing() {
        let root = TempDir::new().unwrap();
        // Create a subdirectory with no plugin files
        make_plugin_dir(&root, "empty-plugin");

        let found = discover_plugins(root.path());
        assert!(found.is_empty());
    }
}
