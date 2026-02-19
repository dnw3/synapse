use async_trait::async_trait;
use regex::Regex;
use std::collections::HashMap;
use std::sync::Arc;
use synaptic_core::SynapticError;
use tokio::sync::RwLock;

use super::{Backend, DirEntry, GrepMatch, GrepOutputMode};

/// In-memory backend for testing and lightweight use.
///
/// Files are stored in a `HashMap<String, String>` keyed by normalized paths.
/// Directories are inferred from path prefixes (no explicit directory entries).
pub struct StateBackend {
    files: Arc<RwLock<HashMap<String, String>>>,
}

impl StateBackend {
    pub fn new() -> Self {
        Self {
            files: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl Default for StateBackend {
    fn default() -> Self {
        Self::new()
    }
}

fn normalize_path(path: &str) -> String {
    let trimmed = path.trim_matches('/');
    if trimmed == "." {
        String::new()
    } else {
        trimmed.to_string()
    }
}

fn glob_to_regex(pattern: &str) -> String {
    let mut regex = String::from("^");
    let mut chars = pattern.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '*' => {
                if chars.peek() == Some(&'*') {
                    chars.next();
                    if chars.peek() == Some(&'/') {
                        chars.next();
                        regex.push_str("(.*/)?");
                    } else {
                        regex.push_str(".*");
                    }
                } else {
                    regex.push_str("[^/]*");
                }
            }
            '?' => regex.push_str("[^/]"),
            '.' => regex.push_str("\\."),
            '{' => regex.push('('),
            '}' => regex.push(')'),
            ',' => regex.push('|'),
            '[' => regex.push('['),
            ']' => regex.push(']'),
            c => regex.push(c),
        }
    }
    regex.push('$');
    regex
}

#[async_trait]
impl Backend for StateBackend {
    async fn ls(&self, path: &str) -> Result<Vec<DirEntry>, SynapticError> {
        let files = self.files.read().await;
        let prefix = normalize_path(path);
        let prefix_with_slash = if prefix.is_empty() {
            String::new()
        } else {
            format!("{}/", prefix)
        };

        let mut entries: HashMap<String, bool> = HashMap::new();

        for key in files.keys() {
            let rel = if prefix_with_slash.is_empty() {
                key.clone()
            } else if let Some(rel) = key.strip_prefix(&prefix_with_slash) {
                rel.to_string()
            } else {
                continue;
            };

            if let Some(slash_pos) = rel.find('/') {
                entries.insert(rel[..slash_pos].to_string(), true);
            } else if !rel.is_empty() {
                entries.entry(rel).or_insert(false);
            }
        }

        let mut result: Vec<DirEntry> = entries
            .into_iter()
            .map(|(name, is_dir)| DirEntry {
                name,
                is_dir,
                size: None,
            })
            .collect();
        result.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(result)
    }

    async fn read_file(
        &self,
        path: &str,
        offset: usize,
        limit: usize,
    ) -> Result<String, SynapticError> {
        let files = self.files.read().await;
        let normalized = normalize_path(path);
        let content = files
            .get(&normalized)
            .ok_or_else(|| SynapticError::Tool(format!("file not found: {}", path)))?;

        let lines: Vec<&str> = content.lines().collect();
        let total = lines.len();

        if offset >= total {
            return Ok(String::new());
        }

        let end = (offset + limit).min(total);
        Ok(lines[offset..end].join("\n"))
    }

    async fn write_file(&self, path: &str, content: &str) -> Result<(), SynapticError> {
        let mut files = self.files.write().await;
        files.insert(normalize_path(path), content.to_string());
        Ok(())
    }

    async fn edit_file(
        &self,
        path: &str,
        old_text: &str,
        new_text: &str,
        replace_all: bool,
    ) -> Result<(), SynapticError> {
        let mut files = self.files.write().await;
        let normalized = normalize_path(path);
        let content = files
            .get(&normalized)
            .ok_or_else(|| SynapticError::Tool(format!("file not found: {}", path)))?
            .clone();

        if !content.contains(old_text) {
            return Err(SynapticError::Tool(format!(
                "old_string not found in {}",
                path
            )));
        }

        let new_content = if replace_all {
            content.replace(old_text, new_text)
        } else {
            content.replacen(old_text, new_text, 1)
        };

        files.insert(normalized, new_content);
        Ok(())
    }

    async fn glob(&self, pattern: &str, base: &str) -> Result<Vec<String>, SynapticError> {
        let files = self.files.read().await;
        let base_normalized = normalize_path(base);

        let regex_str = glob_to_regex(pattern);
        let re = Regex::new(&regex_str)
            .map_err(|e| SynapticError::Tool(format!("invalid glob pattern: {}", e)))?;

        let mut matches = Vec::new();
        for key in files.keys() {
            let rel = if base_normalized.is_empty() {
                key.clone()
            } else if let Some(rel) = key.strip_prefix(&format!("{}/", base_normalized)) {
                rel.to_string()
            } else {
                continue;
            };

            if re.is_match(&rel) {
                matches.push(key.clone());
            }
        }
        matches.sort();
        Ok(matches)
    }

    async fn grep(
        &self,
        pattern: &str,
        path: Option<&str>,
        file_glob: Option<&str>,
        output_mode: GrepOutputMode,
    ) -> Result<String, SynapticError> {
        let files = self.files.read().await;
        let re = Regex::new(pattern)
            .map_err(|e| SynapticError::Tool(format!("invalid regex: {}", e)))?;

        let glob_re = file_glob.and_then(|g| Regex::new(&glob_to_regex(g)).ok());
        let base = path.map(normalize_path).unwrap_or_default();

        let mut file_matches: Vec<GrepMatch> = Vec::new();
        let mut match_files: Vec<String> = Vec::new();
        let mut match_counts: HashMap<String, usize> = HashMap::new();

        for (file_path, content) in files.iter() {
            if !base.is_empty() && !file_path.starts_with(&base) {
                continue;
            }

            if let Some(ref gre) = glob_re {
                let rel = if base.is_empty() {
                    file_path.clone()
                } else {
                    file_path
                        .strip_prefix(&format!("{}/", base))
                        .unwrap_or(file_path)
                        .to_string()
                };
                if !gre.is_match(&rel) {
                    continue;
                }
            }

            let mut found = false;
            for (line_num, line) in content.lines().enumerate() {
                if re.is_match(line) {
                    found = true;
                    file_matches.push(GrepMatch {
                        file: file_path.clone(),
                        line_number: line_num + 1,
                        line: line.to_string(),
                    });
                    *match_counts.entry(file_path.clone()).or_insert(0) += 1;
                }
            }
            if found {
                match_files.push(file_path.clone());
            }
        }

        match output_mode {
            GrepOutputMode::FilesWithMatches => {
                match_files.sort();
                Ok(match_files.join("\n"))
            }
            GrepOutputMode::Content => {
                file_matches
                    .sort_by(|a, b| a.file.cmp(&b.file).then(a.line_number.cmp(&b.line_number)));
                Ok(file_matches
                    .iter()
                    .map(|m| format!("{}:{}:{}", m.file, m.line_number, m.line))
                    .collect::<Vec<_>>()
                    .join("\n"))
            }
            GrepOutputMode::Count => {
                let mut counts: Vec<_> = match_counts.into_iter().collect();
                counts.sort_by(|a, b| a.0.cmp(&b.0));
                Ok(counts
                    .iter()
                    .map(|(f, c)| format!("{}:{}", f, c))
                    .collect::<Vec<_>>()
                    .join("\n"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_to_regex() {
        assert_eq!(glob_to_regex("*.rs"), "^[^/]*\\.rs$");
        assert_eq!(glob_to_regex("**/*.rs"), "^(.*/)?[^/]*\\.rs$");
    }
}
