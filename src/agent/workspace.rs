use std::path::Path;

use super::templates::{find_template, WORKSPACE_TEMPLATES};

/// Parsed identity from IDENTITY.md frontmatter.
#[derive(Debug, Clone, Default, serde::Serialize)]
pub struct IdentityInfo {
    pub name: Option<String>,
    pub emoji: Option<String>,
    pub avatar_url: Option<String>,
    pub theme_color: Option<String>,
}

/// Initialize workspace by copying default templates for files that don't exist.
///
/// Only triggers when SOUL.md is absent (fresh workspace). Returns list of created files.
pub fn initialize_workspace(cwd: &Path) -> Vec<String> {
    // Only initialize if SOUL.md doesn't exist (avoid clobbering existing workspaces)
    if cwd.join("SOUL.md").exists() {
        return Vec::new();
    }

    let mut created = Vec::new();
    for tmpl in WORKSPACE_TEMPLATES {
        let target = cwd.join(tmpl.filename);
        if !target.exists() {
            if let Err(e) = std::fs::write(&target, tmpl.default_content) {
                tracing::warn!(file = %tmpl.filename, error = %e, "Failed to create workspace template");
                continue;
            }
            created.push(tmpl.filename.to_string());
        }
    }

    if !created.is_empty() {
        tracing::info!(
            count = created.len(),
            files = %created.join(", "),
            "Initialized workspace with templates"
        );
    }

    created
}

/// Delete BOOTSTRAP.md after the first session completes.
pub fn delete_bootstrap(cwd: &Path) {
    let path = cwd.join("BOOTSTRAP.md");
    if path.exists() {
        if let Err(e) = std::fs::remove_file(&path) {
            tracing::warn!(error = %e, "Failed to delete BOOTSTRAP.md");
        } else {
            tracing::info!("Deleted BOOTSTRAP.md (first session complete)");
        }
    }
}

/// Parse IDENTITY.md content to extract frontmatter fields.
///
/// Expects YAML frontmatter between `---` delimiters:
/// ```text
/// ---
/// name: Synapse
/// emoji: ⚡
/// avatar_url: https://...
/// theme_color: #6366f1
/// ---
/// ```
pub fn parse_identity(content: &str) -> IdentityInfo {
    let mut info = IdentityInfo::default();

    // Extract frontmatter between --- delimiters
    let trimmed = content.trim();
    if !trimmed.starts_with("---") {
        return info;
    }

    let after_first = &trimmed[3..];
    let end = match after_first.find("---") {
        Some(pos) => pos,
        None => return info,
    };

    let frontmatter = &after_first[..end];

    for line in frontmatter.lines() {
        let line = line.trim();
        if let Some((key, value)) = line.split_once(':') {
            let key = key.trim();
            let value = value.trim();
            if value.is_empty() {
                continue;
            }
            match key {
                "name" => info.name = Some(value.to_string()),
                "emoji" => info.emoji = Some(value.to_string()),
                "avatar_url" => info.avatar_url = Some(value.to_string()),
                "theme_color" => info.theme_color = Some(value.to_string()),
                _ => {}
            }
        }
    }

    info
}

/// Get the default content for a template file.
pub fn default_content_for(filename: &str) -> Option<&'static str> {
    find_template(filename).map(|t| t.default_content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_identity_basic() {
        let content = r#"---
name: MyBot
emoji: 🤖
avatar_url: https://example.com/avatar.png
theme_color: #ff6600
---
# Identity
"#;
        let info = parse_identity(content);
        assert_eq!(info.name.as_deref(), Some("MyBot"));
        assert_eq!(info.emoji.as_deref(), Some("🤖"));
        assert_eq!(info.avatar_url.as_deref(), Some("https://example.com/avatar.png"));
        assert_eq!(info.theme_color.as_deref(), Some("#ff6600"));
    }

    #[test]
    fn test_parse_identity_empty_values() {
        let content = "---\nname:\nemoji: ⚡\n---\n";
        let info = parse_identity(content);
        assert!(info.name.is_none());
        assert_eq!(info.emoji.as_deref(), Some("⚡"));
    }

    #[test]
    fn test_parse_identity_no_frontmatter() {
        let info = parse_identity("# Just markdown");
        assert!(info.name.is_none());
        assert!(info.emoji.is_none());
    }
}
