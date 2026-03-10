use std::path::Path;

use crate::config::ContextConfig;

/// Load project-scoped context files from the working directory.
///
/// Searches for AGENTS.md, MEMORY.md, README.md, and .synapse/context.md in
/// the given directory and returns their concatenated contents for injection
/// into the system prompt. Respects truncation limits from ContextConfig.
pub fn load_project_context(cwd: &Path, ctx_config: &ContextConfig) -> String {
    let candidates = [
        (cwd.join("IDENTITY.md"), "IDENTITY.md"),
        (cwd.join("SOUL.md"), "SOUL.md"),
        (cwd.join("AGENTS.md"), "AGENTS.md"),
        (cwd.join("MEMORY.md"), "MEMORY.md"),
        (cwd.join("USER.md"), "USER.md"),
        (cwd.join("TOOLS.md"), "TOOLS.md"),
        (cwd.join("BOOTSTRAP.md"), "BOOTSTRAP.md"),
        (
            cwd.join(".synapse").join("context.md"),
            ".synapse/context.md",
        ),
        (cwd.join("README.md"), "README.md"),
    ];

    let mut context = String::new();
    let mut total_chars = 0usize;

    for (path, label) in &candidates {
        if path.exists() {
            if let Ok(mut content) = std::fs::read_to_string(path) {
                if content.trim().is_empty() {
                    continue;
                }

                // Per-file truncation
                if ctx_config.max_chars_per_file > 0 && content.len() > ctx_config.max_chars_per_file {
                    content.truncate(ctx_config.max_chars_per_file);
                    content.push_str("\n... [truncated]");
                    tracing::warn!(
                        file = %label,
                        max_chars = ctx_config.max_chars_per_file,
                        "Context file truncated"
                    );
                }

                // Total budget check
                if ctx_config.total_max_chars > 0 && total_chars + content.len() > ctx_config.total_max_chars {
                    let remaining = ctx_config.total_max_chars.saturating_sub(total_chars);
                    if remaining < 100 {
                        tracing::warn!(
                            file = %label,
                            "Skipping context file (total context budget exceeded)"
                        );
                        continue;
                    }
                    content.truncate(remaining);
                    content.push_str("\n... [truncated, budget limit]");
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
