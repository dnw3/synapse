use colored::Colorize;
use synaptic::core::Message;

/// Render a tool call in a compact, readable format.
pub fn render_tool_call(name: &str, args: &serde_json::Value) {
    match name {
        "read_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            let offset = args.get("offset").and_then(|v| v.as_u64()).unwrap_or(0);
            let limit = args.get("limit").and_then(|v| v.as_u64()).unwrap_or(2000);
            if offset > 0 || limit != 2000 {
                eprintln!(
                    "  {} {} ({}-{})",
                    "[read]".cyan(),
                    path,
                    offset,
                    offset + limit
                );
            } else {
                eprintln!("  {} {}", "[read]".cyan(), path);
            }
        }
        "write_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            let content = args.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let lines = content.lines().count();
            eprintln!("  {} {} ({} lines)", "[write]".green(), path, lines);
        }
        "edit_file" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            let old = args.get("old_str").and_then(|v| v.as_str());
            let new = args.get("new_str").and_then(|v| v.as_str());
            eprintln!("  {} {}", "[edit]".yellow(), path);

            // Show inline diff if old/new content available
            if let (Some(old_str), Some(new_str)) = (old, new) {
                render_inline_diff(old_str, new_str);
            }
        }
        "execute" => {
            let cmd = args.get("command").and_then(|v| v.as_str()).unwrap_or("?");
            let display_cmd = if cmd.len() > 80 {
                format!("{}...", &cmd[..77])
            } else {
                cmd.to_string()
            };
            eprintln!("  {} {}", "[exec]".magenta(), display_cmd);
        }
        "ls" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or(".");
            eprintln!("  {} {}", "[ls]".cyan(), path);
        }
        "glob" => {
            let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("?");
            eprintln!("  {} {}", "[glob]".cyan(), pattern);
        }
        "grep" => {
            let pattern = args.get("pattern").and_then(|v| v.as_str()).unwrap_or("?");
            eprintln!("  {} {}", "[grep]".cyan(), pattern);
        }
        "apply_patch" => {
            let patch = args.get("patch").and_then(|v| v.as_str()).unwrap_or("");
            let file_count = patch.matches("+++ ").count();
            let hunk_count = patch.matches("@@ ").count();
            eprintln!(
                "  {} {} file(s), {} hunk(s)",
                "[patch]".yellow(),
                file_count,
                hunk_count
            );
        }
        "read_pdf" => {
            let path = args.get("path").and_then(|v| v.as_str()).unwrap_or("?");
            let page = args.get("page").and_then(|v| v.as_u64());
            if let Some(p) = page {
                eprintln!("  {} {} (page {})", "[pdf]".cyan(), path, p);
            } else {
                eprintln!("  {} {}", "[pdf]".cyan(), path);
            }
        }
        "firecrawl_scrape" => {
            let url = args.get("url").and_then(|v| v.as_str()).unwrap_or("?");
            eprintln!("  {} {}", "[scrape]".cyan(), url);
        }
        "delegate" => {
            let task = args
                .get("task")
                .and_then(|v| v.as_str())
                .unwrap_or("sub-task");
            let role = args
                .get("role")
                .and_then(|v| v.as_str())
                .unwrap_or("agent");
            let short_task = if task.len() > 50 {
                format!("{}...", &task[..47])
            } else {
                task.to_string()
            };
            eprintln!("  {} [{}] {}", "[delegate]".blue(), role, short_task);
        }
        "task" => {
            let desc = args
                .get("task")
                .and_then(|v| v.as_str())
                .unwrap_or("subagent");
            let short = if desc.len() > 60 {
                format!("{}...", &desc[..57])
            } else {
                desc.to_string()
            };
            eprintln!("  {} {}", "[task]".blue(), short);
        }
        _ => {
            eprintln!(
                "  {} {}",
                format!("[{}]", name).dimmed(),
                truncate_json(args)
            );
        }
    }
}

/// Render a tool result (abbreviated).
pub fn render_tool_result(name: &str, content: &str) {
    let status = if content.starts_with("Error") || content.starts_with("error") {
        format!("  {} {}", "✗".red(), truncate(content, 100))
    } else {
        match name {
            "execute" => {
                // Try to show exit code / success
                if content.contains("exit_code: 0") || content.contains("exit code: 0") {
                    format!("  {}", "✓".green())
                } else if content.len() < 100 {
                    format!("  {} {}", "✓".green(), content.trim())
                } else {
                    format!("  {} ({} chars)", "✓".green(), content.len())
                }
            }
            _ => {
                if content.len() > 120 {
                    String::new() // suppress long tool results
                } else {
                    format!("  {} {}", "→".dimmed(), truncate(content, 100))
                }
            }
        }
    };
    if !status.is_empty() {
        eprintln!("{}", status);
    }
}

/// Render new messages from a graph event.
pub fn render_new_messages(messages: &[Message], displayed_count: usize) -> usize {
    let mut count = displayed_count;

    for msg in messages.iter().skip(displayed_count) {
        if msg.is_ai() {
            let tool_calls = msg.tool_calls();
            if !tool_calls.is_empty() {
                for tc in tool_calls {
                    render_tool_call(&tc.name, &tc.arguments);
                }
            } else {
                // Final AI response text (skip NO_REPLY silent turns)
                let content = msg.content();
                if !content.is_empty() && !content.starts_with("NO_REPLY") {
                    eprintln!();
                    println!("{}", content);
                }
            }
        } else if msg.is_tool() {
            let content = msg.content();
            // Try to figure out the tool name from prior messages
            let tool_name = find_tool_name_for_result(messages, count);
            render_tool_result(&tool_name, content);
        }
        count += 1;
    }

    count
}

/// Render an inline diff between old and new content.
///
/// Shows removed lines in red and added lines in green, limited to
/// a reasonable number of lines.
fn render_inline_diff(old: &str, new: &str) {
    let old_lines: Vec<&str> = old.lines().collect();
    let new_lines: Vec<&str> = new.lines().collect();

    let max_display = 8; // limit diff display
    let mut displayed = 0;

    // Show removed lines
    for line in &old_lines {
        if displayed >= max_display {
            eprintln!("    {}", "...".dimmed());
            break;
        }
        if !new_lines.contains(line) {
            eprintln!("    {}", format!("- {}", line).red());
            displayed += 1;
        }
    }

    // Show added lines
    for line in &new_lines {
        if displayed >= max_display {
            eprintln!("    {}", "...".dimmed());
            break;
        }
        if !old_lines.contains(line) {
            eprintln!("    {}", format!("+ {}", line).green());
            displayed += 1;
        }
    }
}

/// Look backwards from a tool result message to find the tool call name.
fn find_tool_name_for_result(messages: &[Message], tool_msg_idx: usize) -> String {
    let tool_call_id = messages
        .get(tool_msg_idx)
        .and_then(|m| m.tool_call_id())
        .unwrap_or_default();

    if tool_call_id.is_empty() {
        return "tool".to_string();
    }

    // Search backwards for the AI message with matching tool_call id
    for msg in messages[..tool_msg_idx].iter().rev() {
        if msg.is_ai() {
            for tc in msg.tool_calls() {
                if tc.id == tool_call_id {
                    return tc.name.clone();
                }
            }
        }
    }
    "tool".to_string()
}

fn truncate(s: &str, max: usize) -> String {
    let s = s.trim();
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max - 3])
    }
}

fn truncate_json(v: &serde_json::Value) -> String {
    let s = v.to_string();
    truncate(&s, 80)
}
