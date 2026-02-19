use async_trait::async_trait;
use regex::Regex;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use synaptic_core::SynapticError;

use super::{Backend, DirEntry, ExecResult, GrepMatch, GrepOutputMode};

/// Real filesystem backend, sandboxed to a root directory.
///
/// All paths are resolved relative to `root`. Path traversal via `..` is rejected.
/// Requires the `filesystem` feature.
pub struct FilesystemBackend {
    root: PathBuf,
}

impl FilesystemBackend {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    fn resolve(&self, path: &str) -> Result<PathBuf, SynapticError> {
        let normalized = path.trim_start_matches('/');
        if normalized.contains("..") {
            return Err(SynapticError::Tool("path traversal rejected".into()));
        }
        Ok(self.root.join(normalized))
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
            c => regex.push(c),
        }
    }
    regex.push('$');
    regex
}

async fn walk_dir(dir: &Path, base: &Path) -> Result<Vec<String>, SynapticError> {
    let mut result = Vec::new();
    let mut stack = vec![dir.to_path_buf()];

    while let Some(current) = stack.pop() {
        let mut entries = tokio::fs::read_dir(&current)
            .await
            .map_err(|e| SynapticError::Tool(format!("read_dir failed: {}", e)))?;

        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| SynapticError::Tool(format!("dir entry failed: {}", e)))?
        {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else if let Ok(rel) = path.strip_prefix(base) {
                result.push(rel.to_string_lossy().to_string());
            }
        }
    }
    result.sort();
    Ok(result)
}

#[async_trait]
impl Backend for FilesystemBackend {
    async fn ls(&self, path: &str) -> Result<Vec<DirEntry>, SynapticError> {
        let full = self.resolve(path)?;
        let mut entries_out = Vec::new();
        let mut rd = tokio::fs::read_dir(&full)
            .await
            .map_err(|e| SynapticError::Tool(format!("ls failed: {}", e)))?;

        while let Some(entry) = rd
            .next_entry()
            .await
            .map_err(|e| SynapticError::Tool(format!("dir entry: {}", e)))?
        {
            let meta = entry
                .metadata()
                .await
                .map_err(|e| SynapticError::Tool(format!("metadata: {}", e)))?;
            entries_out.push(DirEntry {
                name: entry.file_name().to_string_lossy().to_string(),
                is_dir: meta.is_dir(),
                size: Some(meta.len()),
            });
        }
        entries_out.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(entries_out)
    }

    async fn read_file(
        &self,
        path: &str,
        offset: usize,
        limit: usize,
    ) -> Result<String, SynapticError> {
        let full = self.resolve(path)?;
        let content = tokio::fs::read_to_string(&full)
            .await
            .map_err(|e| SynapticError::Tool(format!("read failed: {}", e)))?;

        let lines: Vec<&str> = content.lines().collect();
        let total = lines.len();
        if offset >= total {
            return Ok(String::new());
        }
        let end = (offset + limit).min(total);
        Ok(lines[offset..end].join("\n"))
    }

    async fn write_file(&self, path: &str, content: &str) -> Result<(), SynapticError> {
        let full = self.resolve(path)?;
        if let Some(parent) = full.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| SynapticError::Tool(format!("mkdir failed: {}", e)))?;
        }
        tokio::fs::write(&full, content)
            .await
            .map_err(|e| SynapticError::Tool(format!("write failed: {}", e)))
    }

    async fn edit_file(
        &self,
        path: &str,
        old_text: &str,
        new_text: &str,
        replace_all: bool,
    ) -> Result<(), SynapticError> {
        let full = self.resolve(path)?;
        let content = tokio::fs::read_to_string(&full)
            .await
            .map_err(|e| SynapticError::Tool(format!("read failed: {}", e)))?;

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

        tokio::fs::write(&full, new_content)
            .await
            .map_err(|e| SynapticError::Tool(format!("write failed: {}", e)))
    }

    async fn glob(&self, pattern: &str, base: &str) -> Result<Vec<String>, SynapticError> {
        let base_path = self.resolve(base)?;
        let all_files = walk_dir(&base_path, &base_path).await?;

        let regex_str = glob_to_regex(pattern);
        let re = Regex::new(&regex_str)
            .map_err(|e| SynapticError::Tool(format!("invalid glob: {}", e)))?;

        Ok(all_files.into_iter().filter(|f| re.is_match(f)).collect())
    }

    async fn grep(
        &self,
        pattern: &str,
        path: Option<&str>,
        file_glob: Option<&str>,
        output_mode: GrepOutputMode,
    ) -> Result<String, SynapticError> {
        let base_path = self.resolve(path.unwrap_or("."))?;
        let all_files = walk_dir(&base_path, &base_path).await?;

        let re = Regex::new(pattern)
            .map_err(|e| SynapticError::Tool(format!("invalid regex: {}", e)))?;
        let glob_re = file_glob.and_then(|g| Regex::new(&glob_to_regex(g)).ok());

        let mut file_matches: Vec<GrepMatch> = Vec::new();
        let mut match_files: Vec<String> = Vec::new();
        let mut match_counts: HashMap<String, usize> = HashMap::new();

        for file_rel in &all_files {
            if let Some(ref gre) = glob_re {
                if !gre.is_match(file_rel) {
                    continue;
                }
            }

            let full = base_path.join(file_rel);
            let content = match tokio::fs::read_to_string(&full).await {
                Ok(c) => c,
                Err(_) => continue,
            };

            let mut found = false;
            for (line_num, line) in content.lines().enumerate() {
                if re.is_match(line) {
                    found = true;
                    file_matches.push(GrepMatch {
                        file: file_rel.clone(),
                        line_number: line_num + 1,
                        line: line.to_string(),
                    });
                    *match_counts.entry(file_rel.clone()).or_insert(0) += 1;
                }
            }
            if found {
                match_files.push(file_rel.clone());
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

    async fn execute(
        &self,
        command: &str,
        timeout: Option<Duration>,
    ) -> Result<ExecResult, SynapticError> {
        let mut cmd = tokio::process::Command::new("sh");
        cmd.arg("-c").arg(command).current_dir(&self.root);

        let output = if let Some(dur) = timeout {
            tokio::time::timeout(dur, cmd.output())
                .await
                .map_err(|_| SynapticError::Timeout("command timed out".into()))?
                .map_err(|e| SynapticError::Tool(format!("exec failed: {}", e)))?
        } else {
            cmd.output()
                .await
                .map_err(|e| SynapticError::Tool(format!("exec failed: {}", e)))?
        };

        Ok(ExecResult {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code().unwrap_or(-1),
        })
    }

    fn supports_execution(&self) -> bool {
        true
    }
}
