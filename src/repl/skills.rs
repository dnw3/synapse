/// Result of resolving a skill slash command.
pub enum SkillSlashResult {
    /// Normal skill body to inject as a human message.
    Body(String),
    /// Command dispatch: bypass model, call tool directly.
    ToolDispatch {
        tool_name: String,
        arguments: String,
        arg_mode: String,
    },
}

/// Resolve a skill slash command by searching skill directories.
///
/// Returns the expanded skill body or a tool dispatch, or None.
pub async fn resolve_skill_slash_command(
    name: &str,
    args: &str,
    cwd: &std::path::Path,
) -> Option<SkillSlashResult> {
    use synaptic_deep::middleware::skills::substitute_arguments;

    // Search order: synapse personal > claude personal > project > legacy commands
    let search_dirs = {
        let mut dirs = Vec::new();
        if let Some(home) = dirs::home_dir() {
            dirs.push(home.join(".synapse/skills"));
            dirs.push(home.join(".claude/skills"));
        }
        dirs.push(cwd.join(".claude/skills"));
        dirs.push(cwd.join(".claude/commands"));
        dirs
    };

    for dir in &search_dirs {
        // Try directory with SKILL.md
        let skill_md = dir.join(name).join("SKILL.md");
        if skill_md.exists() {
            let content = std::fs::read_to_string(&skill_md).ok()?;
            let (fm_body, user_invocable, dispatch) = parse_skill_for_slash(&content)?;
            if !user_invocable {
                return None;
            }
            if let Some((tool_name, arg_mode)) = dispatch {
                return Some(SkillSlashResult::ToolDispatch {
                    tool_name,
                    arguments: args.to_string(),
                    arg_mode,
                });
            }
            let expanded = substitute_arguments(&fm_body, args);
            return Some(SkillSlashResult::Body(
                resolve_inline_commands(&expanded, cwd).await,
            ));
        }

        // Try flat .md file (legacy commands/)
        let flat_md = dir.join(format!("{}.md", name));
        if flat_md.exists() {
            let content = std::fs::read_to_string(&flat_md).ok()?;
            // May or may not have frontmatter
            let body = if content.starts_with("---") {
                if let Some((b, invocable, dispatch)) = parse_skill_for_slash(&content) {
                    if !invocable {
                        return None;
                    }
                    if let Some((tool_name, arg_mode)) = dispatch {
                        return Some(SkillSlashResult::ToolDispatch {
                            tool_name,
                            arguments: args.to_string(),
                            arg_mode,
                        });
                    }
                    b
                } else {
                    content
                }
            } else {
                content
            };
            let expanded = substitute_arguments(&body, args);
            return Some(SkillSlashResult::Body(
                resolve_inline_commands(&expanded, cwd).await,
            ));
        }
    }

    None
}

/// Parse frontmatter and return (body, user_invocable, command_dispatch).
///
/// `command_dispatch` is `Some((tool_name, arg_mode))` when `command-dispatch: tool` is set.
fn parse_skill_for_slash(content: &str) -> Option<(String, bool, Option<(String, String)>)> {
    let content = content.trim_start_matches('\u{feff}');
    let mut lines = content.lines();
    if lines.next()?.trim() != "---" {
        return Some((content.to_string(), true, None));
    }

    let mut body = String::new();
    let mut in_body = false;
    let mut user_invocable = true;
    let mut command_dispatch: Option<String> = None;
    let mut command_tool: Option<String> = None;
    let mut command_arg_mode: Option<String> = None;

    for line in lines {
        if !in_body {
            if line.trim() == "---" {
                in_body = true;
                continue;
            }
            let trimmed = line.trim();
            if trimmed.starts_with("user-invocable:") || trimmed.starts_with("user_invocable:") {
                if let Some((_, val)) = trimmed.split_once(':') {
                    user_invocable = val.trim() != "false";
                }
            } else if trimmed.starts_with("command-dispatch:")
                || trimmed.starts_with("command_dispatch:")
            {
                if let Some((_, val)) = trimmed.split_once(':') {
                    command_dispatch = Some(val.trim().to_string());
                }
            } else if trimmed.starts_with("command-tool:") || trimmed.starts_with("command_tool:") {
                if let Some((_, val)) = trimmed.split_once(':') {
                    command_tool = Some(val.trim().to_string());
                }
            } else if trimmed.starts_with("command-arg-mode:")
                || trimmed.starts_with("command_arg_mode:")
            {
                if let Some((_, val)) = trimmed.split_once(':') {
                    command_arg_mode = Some(val.trim().to_string());
                }
            }
        } else {
            if !body.is_empty() {
                body.push('\n');
            }
            body.push_str(line);
        }
    }

    let dispatch = if command_dispatch.as_deref() == Some("tool") {
        Some((
            command_tool.unwrap_or_default(),
            command_arg_mode.unwrap_or_else(|| "passthrough".to_string()),
        ))
    } else {
        None
    };

    Some((body, user_invocable, dispatch))
}

/// Resolve !`command` placeholders by executing shell commands.
async fn resolve_inline_commands(body: &str, cwd: &std::path::Path) -> String {
    let mut result = body.to_string();
    while let Some(start) = result.find("!`") {
        let after = start + 2;
        if let Some(end) = result[after..].find('`') {
            let command = result[after..after + end].to_string();
            let output = match tokio::process::Command::new("sh")
                .arg("-c")
                .arg(&command)
                .current_dir(cwd)
                .output()
                .await
            {
                Ok(o) => String::from_utf8_lossy(&o.stdout).trim().to_string(),
                Err(e) => format!("[error: {}]", e),
            };
            result = format!(
                "{}{}{}",
                &result[..start],
                output,
                &result[after + end + 1..]
            );
        } else {
            break;
        }
    }
    result
}
