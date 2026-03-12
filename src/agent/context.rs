use std::path::Path;

use crate::config::ContextConfig;

/// Smart truncation: keep 70% head + 30% tail with a separator.
/// Breaks at line boundaries for cleaner output.
fn truncate_smart(content: &str, max_chars: usize) -> String {
    if content.len() <= max_chars {
        return content.to_string();
    }

    // Reserve ~30 chars for the separator line
    let separator_reserve = 30;
    if max_chars <= separator_reserve + 20 {
        // Too small for head+tail — just take the head
        let mut end = max_chars.min(content.len());
        while end > 0 && !content.is_char_boundary(end) {
            end -= 1;
        }
        return format!("{}\n... [truncated]", &content[..end]);
    }

    let usable = max_chars - separator_reserve;
    let mut head_budget = (usable as f64 * 0.7) as usize;
    let tail_budget = usable - head_budget;

    // Ensure head_budget lands on a char boundary
    while head_budget > 0 && !content.is_char_boundary(head_budget) {
        head_budget -= 1;
    }

    // Snap to nearest line break
    let head_end = content[..head_budget].rfind('\n').unwrap_or(head_budget);

    let mut tail_start_raw = content.len().saturating_sub(tail_budget);
    // Ensure tail_start_raw lands on a char boundary
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

/// Load workspace context files from the workspace directory.
///
/// Searches for IDENTITY.md, SOUL.md, AGENTS.md, etc. in the workspace
/// directory and returns their concatenated contents for injection into
/// the system prompt. Respects truncation limits from ContextConfig.
///
/// The `workspace_dir` is the dedicated workspace (e.g. `~/.synapse/workspace/`),
/// while `project_dir` is the CWD where the agent operates (for README.md fallback).
pub fn load_project_context(
    workspace_dir: &Path,
    project_dir: &Path,
    ctx_config: &ContextConfig,
) -> String {
    // Workspace context files (from ~/.synapse/workspace/)
    let mut candidates: Vec<(std::path::PathBuf, &str)> = vec![
        (workspace_dir.join("IDENTITY.md"), "IDENTITY.md"),
        (workspace_dir.join("SOUL.md"), "SOUL.md"),
        (workspace_dir.join("AGENTS.md"), "AGENTS.md"),
        (workspace_dir.join("MEMORY.md"), "MEMORY.md"),
        (workspace_dir.join("USER.md"), "USER.md"),
        (workspace_dir.join("TOOLS.md"), "TOOLS.md"),
        (workspace_dir.join("BOOTSTRAP.md"), "BOOTSTRAP.md"),
    ];

    // Project-level context (from CWD — the actual project directory)
    candidates.push((project_dir.join("CLAUDE.md"), "CLAUDE.md"));
    candidates.push((
        project_dir.join(".synapse").join("context.md"),
        ".synapse/context.md",
    ));
    candidates.push((project_dir.join("README.md"), "README.md"));

    let mut context = String::new();
    let mut total_chars = 0usize;

    for (path, label) in &candidates {
        if path.exists() {
            if let Ok(mut content) = std::fs::read_to_string(path) {
                if content.trim().is_empty() {
                    continue;
                }

                // Per-file truncation (head+tail strategy)
                if ctx_config.max_chars_per_file > 0
                    && content.len() > ctx_config.max_chars_per_file
                {
                    content = truncate_smart(&content, ctx_config.max_chars_per_file);
                    tracing::warn!(
                        file = %label,
                        max_chars = ctx_config.max_chars_per_file,
                        "Context file truncated"
                    );
                }

                // Total budget check
                if ctx_config.total_max_chars > 0
                    && total_chars + content.len() > ctx_config.total_max_chars
                {
                    let remaining = ctx_config.total_max_chars.saturating_sub(total_chars);
                    if remaining < 100 {
                        tracing::warn!(
                            file = %label,
                            "Skipping context file (total context budget exceeded)"
                        );
                        continue;
                    }
                    content = truncate_smart(&content, remaining);
                }

                if !context.is_empty() {
                    context.push_str("\n\n");
                }
                total_chars += content.len();
                context.push_str(&format!("--- {} ---\n{}", label, content.trim()));
                tracing::debug!(file = %label, "Loaded project context");
            }
        }
    }

    context
}
