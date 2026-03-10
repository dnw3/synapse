//! Apply Patch tool for the Deep Agent.
//!
//! Accepts a unified diff and applies it to files on disk.
//! Complements the built-in `edit_file` tool with batch patch support.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use synaptic::core::{SynapticError, Tool};

/// Tool that applies a unified diff patch to files.
pub struct ApplyPatchTool {
    work_dir: PathBuf,
}

impl ApplyPatchTool {
    pub fn new(work_dir: &Path) -> Arc<dyn Tool> {
        Arc::new(Self {
            work_dir: work_dir.to_path_buf(),
        })
    }
}

#[async_trait]
impl Tool for ApplyPatchTool {
    fn name(&self) -> &'static str {
        "apply_patch"
    }

    fn description(&self) -> &'static str {
        "Apply a unified diff patch to one or more files. The patch should be in standard unified diff format (output of `diff -u` or `git diff`). Each hunk specifies the file path and line changes."
    }

    fn parameters(&self) -> Option<Value> {
        Some(json!({
            "type": "object",
            "properties": {
                "patch": {
                    "type": "string",
                    "description": "The unified diff patch content to apply."
                }
            },
            "required": ["patch"]
        }))
    }

    async fn call(&self, args: Value) -> Result<Value, SynapticError> {
        let patch_text = args
            .get("patch")
            .and_then(|v| v.as_str())
            .ok_or_else(|| SynapticError::Tool("missing 'patch' argument".into()))?;

        let results = apply_unified_diff(&self.work_dir, patch_text)?;

        if results.is_empty() {
            return Err(SynapticError::Tool(
                "no file changes found in patch".into(),
            ));
        }

        let summary: Vec<String> = results
            .iter()
            .map(|(path, hunks)| format!("{}: {} hunk(s) applied", path, hunks))
            .collect();

        Ok(Value::String(format!(
            "Patch applied successfully:\n{}",
            summary.join("\n")
        )))
    }
}

/// Parse and apply a unified diff to files on disk.
/// Returns a list of (file_path, hunks_applied) tuples.
fn apply_unified_diff(
    work_dir: &Path,
    patch: &str,
) -> Result<Vec<(String, usize)>, SynapticError> {
    let mut results = Vec::new();
    let mut current_file: Option<String> = None;
    let mut hunks: Vec<Hunk> = Vec::new();
    let mut current_hunk: Option<HunkBuilder> = None;

    for line in patch.lines() {
        if line.starts_with("--- ") {
            // Flush previous file
            if let Some(ref file) = current_file {
                if !hunks.is_empty() {
                    let applied = apply_hunks_to_file(work_dir, file, &hunks)?;
                    results.push((file.clone(), applied));
                    hunks.clear();
                }
            }
            // Parse: "--- a/path/to/file" or "--- path/to/file"
            continue;
        }

        if line.starts_with("+++ ") {
            // Parse target file: "+++ b/path/to/file" or "+++ path/to/file"
            let path = line
                .strip_prefix("+++ b/")
                .or_else(|| line.strip_prefix("+++ "))
                .unwrap_or(&line[4..])
                .trim();
            current_file = Some(path.to_string());
            continue;
        }

        if line.starts_with("@@ ") {
            // Flush current hunk
            if let Some(builder) = current_hunk.take() {
                hunks.push(builder.build());
            }

            // Parse: "@@ -start,count +start,count @@"
            if let Some(hunk_header) = parse_hunk_header(line) {
                current_hunk = Some(HunkBuilder {
                    old_start: hunk_header.0,
                    lines: Vec::new(),
                });
            }
            continue;
        }

        if let Some(ref mut builder) = current_hunk {
            if let Some(stripped) = line.strip_prefix('+') {
                builder.lines.push(DiffLine::Add(stripped.to_string()));
            } else if let Some(stripped) = line.strip_prefix('-') {
                builder.lines.push(DiffLine::Remove(stripped.to_string()));
            } else if let Some(stripped) = line.strip_prefix(' ') {
                builder.lines.push(DiffLine::Context(stripped.to_string()));
            } else if line.is_empty() {
                builder.lines.push(DiffLine::Context(String::new()));
            }
        }
    }

    // Flush last hunk and file
    if let Some(builder) = current_hunk.take() {
        hunks.push(builder.build());
    }
    if let Some(ref file) = current_file {
        if !hunks.is_empty() {
            let applied = apply_hunks_to_file(work_dir, file, &hunks)?;
            results.push((file.clone(), applied));
        }
    }

    Ok(results)
}

#[derive(Debug)]
#[allow(dead_code)]
enum DiffLine {
    Context(String),
    Add(String),
    Remove(String),
}

struct HunkBuilder {
    old_start: usize,
    lines: Vec<DiffLine>,
}

impl HunkBuilder {
    fn build(self) -> Hunk {
        Hunk {
            old_start: self.old_start,
            lines: self.lines,
        }
    }
}

#[derive(Debug)]
struct Hunk {
    old_start: usize,
    lines: Vec<DiffLine>,
}

/// Parse "@@ -old_start,old_count +new_start,new_count @@" header.
fn parse_hunk_header(line: &str) -> Option<(usize, usize)> {
    let line = line.strip_prefix("@@ ")?;
    let line = line.split(" @@").next()?;

    let parts: Vec<&str> = line.split(' ').collect();
    if parts.len() < 2 {
        return None;
    }

    let old_part = parts[0].strip_prefix('-')?;
    let old_start: usize = old_part.split(',').next()?.parse().ok()?;

    let new_part = parts[1].strip_prefix('+')?;
    let new_start: usize = new_part.split(',').next()?.parse().ok()?;

    Some((old_start, new_start))
}

/// Apply hunks to a single file, returning the number of hunks applied.
fn apply_hunks_to_file(
    work_dir: &Path,
    file_path: &str,
    hunks: &[Hunk],
) -> Result<usize, SynapticError> {
    let full_path = work_dir.join(file_path);

    let original = if full_path.exists() {
        std::fs::read_to_string(&full_path)
            .map_err(|e| SynapticError::Tool(format!("failed to read '{}': {}", file_path, e)))?
    } else {
        // New file — start empty
        String::new()
    };

    let mut lines: Vec<String> = original.lines().map(|l| l.to_string()).collect();
    let mut offset: isize = 0;
    let mut applied = 0;

    for hunk in hunks {
        let start_idx = ((hunk.old_start as isize - 1) + offset).max(0) as usize;
        let mut pos = start_idx;
        let mut removals = 0;
        let mut additions = 0;

        for diff_line in &hunk.lines {
            match diff_line {
                DiffLine::Context(_) => {
                    pos += 1;
                }
                DiffLine::Remove(_) => {
                    if pos < lines.len() {
                        lines.remove(pos);
                        removals += 1;
                    }
                }
                DiffLine::Add(text) => {
                    if pos <= lines.len() {
                        lines.insert(pos, text.clone());
                        pos += 1;
                        additions += 1;
                    }
                }
            }
        }

        offset += additions as isize - removals as isize;
        applied += 1;
    }

    // Write result
    if let Some(parent) = full_path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            SynapticError::Tool(format!("failed to create directory: {}", e))
        })?;
    }

    let result = lines.join("\n");
    // Preserve trailing newline if original had one
    let result = if original.ends_with('\n') && !result.ends_with('\n') {
        format!("{}\n", result)
    } else {
        result
    };

    std::fs::write(&full_path, &result)
        .map_err(|e| SynapticError::Tool(format!("failed to write '{}': {}", file_path, e)))?;

    tracing::info!(file = %file_path, "patch applied");

    Ok(applied)
}
