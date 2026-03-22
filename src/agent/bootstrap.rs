use std::path::{Path, PathBuf};

use crate::config::ContextConfig;

/// Category determines which session kinds load this file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BootstrapCategory {
    /// Loaded for Full, Subagent, and Cron sessions.
    Minimal,
    /// Loaded only for Full sessions (BOOTSTRAP.md, MEMORY.md).
    FullOnly,
    /// Loaded for Full and Heartbeat sessions (HEARTBEAT.md).
    Heartbeat,
}

/// Which kind of session is requesting bootstrap context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionKind {
    /// Normal interactive session (web, bot, REPL, CLI).
    Full,
    /// Spawned sub-agent.
    Subagent,
    /// Scheduled/cron task.
    Cron,
    /// Periodic heartbeat poll.
    Heartbeat,
}

/// A loaded bootstrap file with metadata.
#[derive(Debug, Clone)]
pub struct BootstrapFile {
    /// Filename (e.g. "SOUL.md").
    pub name: String,
    /// File content (after truncation).
    pub content: String,
    /// Category for session filtering.
    pub category: BootstrapCategory,
}

/// Standard bootstrap files in load order.
const STANDARD_FILES: &[(&str, BootstrapCategory)] = &[
    ("AGENTS.md", BootstrapCategory::Minimal),
    ("SOUL.md", BootstrapCategory::Minimal),
    ("IDENTITY.md", BootstrapCategory::Minimal),
    ("USER.md", BootstrapCategory::Minimal),
    ("TOOLS.md", BootstrapCategory::Minimal),
    ("HEARTBEAT.md", BootstrapCategory::Heartbeat),
    ("BOOTSTRAP.md", BootstrapCategory::FullOnly),
    ("MEMORY.md", BootstrapCategory::FullOnly),
];

/// Recognized bootstrap filenames (for extra_files validation).
const RECOGNIZED_NAMES: &[&str] = &[
    "AGENTS.md",
    "SOUL.md",
    "IDENTITY.md",
    "USER.md",
    "TOOLS.md",
    "HEARTBEAT.md",
    "BOOTSTRAP.md",
    "MEMORY.md",
    "CLAUDE.md",
];

/// Single-directory bootstrap file loader.
pub struct BootstrapLoader {
    workspace_dir: PathBuf,
    config: ContextConfig,
}

impl BootstrapLoader {
    pub fn new(workspace_dir: PathBuf, config: ContextConfig) -> Self {
        Self {
            workspace_dir,
            config,
        }
    }

    /// Load bootstrap files filtered by session kind.
    pub fn load(&self, session_kind: SessionKind) -> Vec<BootstrapFile> {
        let mut files = Vec::new();
        let mut total_chars = 0usize;

        // 1. Standard files from workspace_dir
        for &(name, category) in STANDARD_FILES {
            if !should_load(category, session_kind) {
                continue;
            }
            let path = self.workspace_dir.join(name);
            if let Some(bf) = self.read_and_truncate(&path, name, category, &mut total_chars) {
                files.push(bf);
            }
        }

        // 2. Extra files from config
        let extra_paths = self.resolve_extra_files();
        for (path, name) in &extra_paths {
            let category = category_for_name(name);
            if !should_load(category, session_kind) {
                continue;
            }
            if let Some(bf) = self.read_and_truncate(path, name, category, &mut total_chars) {
                files.push(bf);
            }
        }

        files
    }

    /// Format loaded bootstrap files for system prompt injection.
    pub fn format_for_prompt(files: &[BootstrapFile]) -> String {
        if files.is_empty() {
            return String::new();
        }
        let mut out = String::from("# Project Context\n");
        for f in files {
            out.push_str(&format!("\n## {}\n{}\n", f.name, f.content.trim()));
        }
        out
    }

    /// Read a file, apply per-file and total truncation, return BootstrapFile or None.
    fn read_and_truncate(
        &self,
        path: &Path,
        name: &str,
        category: BootstrapCategory,
        total_chars: &mut usize,
    ) -> Option<BootstrapFile> {
        if !path.exists() {
            return None;
        }
        let mut content = match std::fs::read_to_string(path) {
            Ok(c) => c,
            Err(e) => {
                tracing::warn!(file = %name, error = %e, "Failed to read bootstrap file");
                return None;
            }
        };
        if content.trim().is_empty() {
            return None;
        }

        // Per-file truncation
        if self.config.max_chars_per_file > 0 && content.len() > self.config.max_chars_per_file {
            content = truncate_smart(&content, self.config.max_chars_per_file);
            tracing::warn!(file = %name, max_chars = self.config.max_chars_per_file, "Bootstrap file truncated");
        }

        // Total budget check
        if self.config.total_max_chars > 0
            && *total_chars + content.len() > self.config.total_max_chars
        {
            let remaining = self.config.total_max_chars.saturating_sub(*total_chars);
            if remaining < 100 {
                tracing::warn!(file = %name, "Skipping bootstrap file (total budget exceeded)");
                return None;
            }
            content = truncate_smart(&content, remaining);
        }

        *total_chars += content.len();
        tracing::debug!(file = %name, chars = content.len(), "Loaded bootstrap file");

        Some(BootstrapFile {
            name: name.to_string(),
            content,
            category,
        })
    }

    /// Resolve extra_files + extra_patterns into (path, filename) pairs.
    /// Validates: recognized filename, canonicalize succeeds, no `..` in raw path.
    fn resolve_extra_files(&self) -> Vec<(PathBuf, String)> {
        let mut results = Vec::new();
        let home = dirs::home_dir().unwrap_or_default();

        // Explicit extra_files
        for raw in &self.config.extra_files {
            if raw.contains("..") {
                tracing::warn!(path = %raw, "Extra file skipped: path contains '..'");
                continue;
            }
            let expanded = expand_tilde(raw, &home);
            let canonical = match expanded.canonicalize() {
                Ok(p) => p,
                Err(e) => {
                    tracing::warn!(path = %raw, error = %e, "Extra file skipped: canonicalize failed");
                    continue;
                }
            };
            if let Some(name) = validated_bootstrap_name(&canonical) {
                results.push((canonical, name));
            } else {
                tracing::warn!(path = %raw, "Extra file skipped: not a recognized bootstrap filename");
            }
        }

        // Glob extra_patterns
        for pattern in &self.config.extra_patterns {
            if pattern.contains("..") {
                tracing::warn!(pattern = %pattern, "Extra pattern skipped: contains '..'");
                continue;
            }
            let expanded = expand_tilde(pattern, &home);
            let pattern_str = expanded.to_string_lossy();
            match glob::glob(&pattern_str) {
                Ok(entries) => {
                    let mut count = 0;
                    for entry in entries.flatten() {
                        if count >= 50 {
                            tracing::warn!(pattern = %pattern, "Extra pattern capped at 50 matches");
                            break;
                        }
                        let canonical = match entry.canonicalize() {
                            Ok(p) => p,
                            Err(_) => continue,
                        };
                        if let Some(name) = validated_bootstrap_name(&canonical) {
                            results.push((canonical, name));
                            count += 1;
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(pattern = %pattern, error = %e, "Invalid glob pattern");
                }
            }
        }

        results
    }
}

/// Check if a category should be loaded for a given session kind.
fn should_load(category: BootstrapCategory, kind: SessionKind) -> bool {
    match kind {
        SessionKind::Full => true,
        SessionKind::Subagent | SessionKind::Cron => category == BootstrapCategory::Minimal,
        SessionKind::Heartbeat => category == BootstrapCategory::Heartbeat,
    }
}

/// Map a filename to its bootstrap category.
fn category_for_name(name: &str) -> BootstrapCategory {
    match name {
        "HEARTBEAT.md" => BootstrapCategory::Heartbeat,
        "BOOTSTRAP.md" | "MEMORY.md" => BootstrapCategory::FullOnly,
        _ => BootstrapCategory::Minimal,
    }
}

/// Validate that a path points to a recognized bootstrap filename.
fn validated_bootstrap_name(path: &Path) -> Option<String> {
    let name = path.file_name()?.to_str()?;
    if RECOGNIZED_NAMES.contains(&name) {
        Some(name.to_string())
    } else {
        None
    }
}

/// Expand `~` to home directory.
fn expand_tilde(raw: &str, home: &Path) -> PathBuf {
    if raw.starts_with("~/") {
        home.join(&raw[2..])
    } else {
        PathBuf::from(raw)
    }
}

/// Smart truncation: keep 70% head + 30% tail with a separator.
fn truncate_smart(content: &str, max_chars: usize) -> String {
    if content.len() <= max_chars {
        return content.to_string();
    }

    let separator_reserve = 30;
    if max_chars <= separator_reserve + 20 {
        let mut end = max_chars.min(content.len());
        while end > 0 && !content.is_char_boundary(end) {
            end -= 1;
        }
        return format!("{}\n... [truncated]", &content[..end]);
    }

    let usable = max_chars - separator_reserve;
    let mut head_budget = (usable as f64 * 0.7) as usize;
    let tail_budget = usable - head_budget;

    while head_budget > 0 && !content.is_char_boundary(head_budget) {
        head_budget -= 1;
    }
    let head_end = content[..head_budget].rfind('\n').unwrap_or(head_budget);

    let mut tail_start_raw = content.len().saturating_sub(tail_budget);
    while tail_start_raw < content.len() && !content.is_char_boundary(tail_start_raw) {
        tail_start_raw += 1;
    }
    let tail_start = content[tail_start_raw..]
        .find('\n')
        .map(|i| tail_start_raw + i + 1)
        .unwrap_or(tail_start_raw);

    let omitted = content.len() - head_end - (content.len() - tail_start);
    format!(
        "{}\n\n... [{} chars truncated] ...\n\n{}",
        &content[..head_end],
        omitted,
        &content[tail_start..]
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_truncate_smart_no_truncation() {
        let content = "short content";
        assert_eq!(truncate_smart(content, 100), content);
    }

    #[test]
    fn test_truncate_smart_head_tail() {
        let content = "a\n".repeat(100); // 200 chars
        let result = truncate_smart(&content, 80);
        assert!(result.contains("chars truncated"));
    }

    #[test]
    fn test_truncate_smart_tiny_budget() {
        let content = "a".repeat(200);
        let result = truncate_smart(&content, 30);
        assert!(result.contains("[truncated]"));
    }

    #[test]
    fn test_session_kind_full_loads_all() {
        let dir = setup_workspace_with_all_files();
        let loader = BootstrapLoader::new(dir.path().to_path_buf(), ContextConfig::default());
        let files = loader.load(SessionKind::Full);
        assert_eq!(files.len(), 8, "Full session should load all 8 files");
        let names: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
        for expected in &[
            "AGENTS.md",
            "SOUL.md",
            "IDENTITY.md",
            "USER.md",
            "TOOLS.md",
            "HEARTBEAT.md",
            "BOOTSTRAP.md",
            "MEMORY.md",
        ] {
            assert!(names.contains(expected), "Missing {expected}");
        }
    }

    #[test]
    fn test_session_kind_subagent_loads_minimal() {
        let dir = setup_workspace_with_all_files();
        let loader = BootstrapLoader::new(dir.path().to_path_buf(), ContextConfig::default());
        let files = loader.load(SessionKind::Subagent);
        let names: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
        assert!(names.contains(&"AGENTS.md"));
        assert!(names.contains(&"SOUL.md"));
        assert!(names.contains(&"IDENTITY.md"));
        assert!(names.contains(&"USER.md"));
        assert!(names.contains(&"TOOLS.md"));
        assert!(!names.contains(&"HEARTBEAT.md"));
        assert!(!names.contains(&"BOOTSTRAP.md"));
        assert!(!names.contains(&"MEMORY.md"));
    }

    #[test]
    fn test_session_kind_heartbeat_loads_only_heartbeat() {
        let dir = setup_workspace_with_all_files();
        let loader = BootstrapLoader::new(dir.path().to_path_buf(), ContextConfig::default());
        let files = loader.load(SessionKind::Heartbeat);
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].name, "HEARTBEAT.md");
    }

    #[test]
    fn test_empty_workspace_returns_empty() {
        let dir = tempfile::tempdir().unwrap();
        let loader = BootstrapLoader::new(dir.path().to_path_buf(), ContextConfig::default());
        let files = loader.load(SessionKind::Full);
        assert!(files.is_empty());
    }

    #[test]
    fn test_empty_file_skipped() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("SOUL.md"), "  \n  ").unwrap();
        let loader = BootstrapLoader::new(dir.path().to_path_buf(), ContextConfig::default());
        let files = loader.load(SessionKind::Full);
        assert!(files.is_empty());
    }

    #[test]
    fn test_extra_files_loaded() {
        let workspace = setup_workspace_with_all_files();
        let extra_dir = tempfile::tempdir().unwrap();
        std::fs::write(
            extra_dir.path().join("CLAUDE.md"),
            "# Claude\nProject instructions",
        )
        .unwrap();

        let config = ContextConfig {
            extra_files: vec![extra_dir
                .path()
                .join("CLAUDE.md")
                .to_string_lossy()
                .to_string()],
            ..Default::default()
        };
        let loader = BootstrapLoader::new(workspace.path().to_path_buf(), config);
        let files = loader.load(SessionKind::Full);
        let names: Vec<&str> = files.iter().map(|f| f.name.as_str()).collect();
        assert!(
            names.contains(&"CLAUDE.md"),
            "Extra CLAUDE.md should be loaded"
        );
        assert_eq!(files.len(), 9); // 8 standard + 1 extra
    }

    #[test]
    fn test_extra_file_unrecognized_name_skipped() {
        let workspace = tempfile::tempdir().unwrap();
        let extra_dir = tempfile::tempdir().unwrap();
        std::fs::write(extra_dir.path().join("RANDOM.md"), "# Random").unwrap();

        let config = ContextConfig {
            extra_files: vec![extra_dir
                .path()
                .join("RANDOM.md")
                .to_string_lossy()
                .to_string()],
            ..Default::default()
        };
        let loader = BootstrapLoader::new(workspace.path().to_path_buf(), config);
        let files = loader.load(SessionKind::Full);
        assert!(files.is_empty(), "Unrecognized filename should be skipped");
    }

    #[test]
    fn test_extra_file_dotdot_rejected() {
        let config = ContextConfig {
            extra_files: vec!["../etc/AGENTS.md".to_string()],
            ..Default::default()
        };
        let dir = tempfile::tempdir().unwrap();
        let loader = BootstrapLoader::new(dir.path().to_path_buf(), config);
        let files = loader.load(SessionKind::Full);
        assert!(files.is_empty(), "Paths with .. should be rejected");
    }

    #[test]
    fn test_format_for_prompt_output() {
        let files = vec![
            BootstrapFile {
                name: "SOUL.md".into(),
                content: "Be helpful.".into(),
                category: BootstrapCategory::Minimal,
            },
            BootstrapFile {
                name: "USER.md".into(),
                content: "Name: Alice".into(),
                category: BootstrapCategory::Minimal,
            },
        ];
        let result = BootstrapLoader::format_for_prompt(&files);
        assert!(result.starts_with("# Project Context\n"));
        assert!(result.contains("## SOUL.md\nBe helpful."));
        assert!(result.contains("## USER.md\nName: Alice"));
    }

    #[test]
    fn test_format_for_prompt_empty() {
        let result = BootstrapLoader::format_for_prompt(&[]);
        assert!(result.is_empty());
    }

    fn setup_workspace_with_all_files() -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for name in &[
            "AGENTS.md",
            "SOUL.md",
            "IDENTITY.md",
            "USER.md",
            "TOOLS.md",
            "HEARTBEAT.md",
            "BOOTSTRAP.md",
            "MEMORY.md",
        ] {
            std::fs::write(dir.path().join(name), format!("# {name}\nContent")).unwrap();
        }
        dir
    }
}
