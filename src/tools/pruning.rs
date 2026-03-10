//! Tool result pruning — two-phase truncation of large tool outputs.
//!
//! Phase 1 (soft-trim): Preserves head + tail with a truncation marker.
//! Phase 2 (hard-clear): Replaces entire content with a placeholder.
//!
//! Supports:
//! - Configurable head/tail char ratio for soft-trim
//! - keepLastAssistants: protect tool results from the N most recent assistant turns
//! - Tool allow/deny filters: skip pruning for specific tools

use synaptic::core::Message;

/// Configuration for tool result pruning.
pub struct PruningOptions {
    /// Soft-trim threshold (0 = disabled).
    pub max_chars: usize,
    /// Hard-clear threshold (0 = disabled).
    pub hard_clear_chars: usize,
    /// Chars to keep from the beginning in soft-trim. 0 = auto (half of max_chars).
    pub head_chars: usize,
    /// Chars to keep from the end in soft-trim. 0 = auto (half of max_chars).
    pub tail_chars: usize,
    /// Number of most-recent assistant turns whose tool results are protected from pruning.
    pub keep_last_assistants: usize,
    /// Tool names to exclude from pruning (allow list). Empty = prune all.
    pub allow_tools: Vec<String>,
    /// Tool names to always prune (deny list). Takes precedence over allow.
    pub deny_tools: Vec<String>,
}

impl Default for PruningOptions {
    fn default() -> Self {
        Self {
            max_chars: 8000,
            hard_clear_chars: 32000,
            head_chars: 0,
            tail_chars: 0,
            keep_last_assistants: 0,
            allow_tools: Vec::new(),
            deny_tools: Vec::new(),
        }
    }
}

impl PruningOptions {
    pub fn from_config(config: &crate::config::MemoryConfig) -> Self {
        Self {
            max_chars: config.max_tool_result_chars,
            hard_clear_chars: config.hard_clear_chars,
            head_chars: config.soft_trim_head_chars,
            tail_chars: config.soft_trim_tail_chars,
            keep_last_assistants: config.keep_last_assistants,
            allow_tools: config.prune_allow_tools.clone(),
            deny_tools: config.prune_deny_tools.clone(),
        }
    }
}

/// Prune tool results that exceed size limits.
///
/// - `max_chars`: soft-trim threshold — head+tail with truncation marker.
/// - `hard_clear_chars`: hard-clear threshold — entire content replaced with placeholder.
///
/// Hard-clear is applied first (larger threshold), then soft-trim (smaller threshold).
pub fn prune_tool_results(messages: &mut Vec<Message>, max_chars: usize, hard_clear_chars: usize) {
    prune_tool_results_with_options(
        messages,
        &PruningOptions {
            max_chars,
            hard_clear_chars,
            ..Default::default()
        },
    );
}

/// Prune tool results with full configuration options.
pub fn prune_tool_results_with_options(messages: &mut Vec<Message>, opts: &PruningOptions) {
    if opts.max_chars == 0 && opts.hard_clear_chars == 0 {
        return;
    }

    // Build set of protected tool_call_ids (from keepLastAssistants)
    let protected_ids = if opts.keep_last_assistants > 0 {
        collect_protected_tool_ids(messages, opts.keep_last_assistants)
    } else {
        Vec::new()
    };

    for i in 0..messages.len() {
        if !messages[i].is_tool() {
            continue;
        }

        let tool_call_id = messages[i]
            .tool_call_id()
            .unwrap_or("")
            .to_string();

        // Skip if this tool result is protected by keepLastAssistants
        if !protected_ids.is_empty() && protected_ids.contains(&tool_call_id) {
            continue;
        }

        // Check tool allow/deny filters
        let tool_name = find_tool_name_by_id(messages, i, &tool_call_id);
        if should_skip_pruning(&tool_name, &opts.allow_tools, &opts.deny_tools) {
            continue;
        }

        let content = messages[i].content();
        let len = content.len();

        // Phase 2: hard-clear — replace entirely if very large
        if opts.hard_clear_chars > 0 && len > opts.hard_clear_chars {
            let pruned = format!("[Tool result cleared ({} chars)]", len);
            messages[i] = Message::tool(pruned, &tool_call_id);
            continue;
        }

        // Phase 1: soft-trim — head + tail with configurable ratio
        if opts.max_chars > 0 && len > opts.max_chars {
            let (head_size, tail_size) = if opts.head_chars > 0 || opts.tail_chars > 0 {
                // Use configured head/tail sizes, capped to max_chars
                let h = opts.head_chars.min(opts.max_chars);
                let t = opts.tail_chars.min(opts.max_chars.saturating_sub(h));
                (h, t)
            } else {
                // Default: 50/50 split
                let half = opts.max_chars / 2;
                (half, half)
            };

            let head = &content[..head_size.min(len)];
            let tail = &content[len.saturating_sub(tail_size)..];
            let truncated_count = len - head_size - tail_size;
            let pruned = format!(
                "{}\n...\n[{} chars truncated]\n...\n{}",
                head, truncated_count, tail
            );
            messages[i] = Message::tool(pruned, &tool_call_id);
        }
    }
}

/// Collect tool_call_ids from the last N assistant turns (to protect from pruning).
fn collect_protected_tool_ids(messages: &[Message], keep_last: usize) -> Vec<String> {
    let mut ids = Vec::new();
    let mut assistant_count = 0;

    for msg in messages.iter().rev() {
        if msg.is_ai() {
            let tool_calls = msg.tool_calls();
            if !tool_calls.is_empty() {
                assistant_count += 1;
                if assistant_count > keep_last {
                    break;
                }
                for tc in tool_calls {
                    ids.push(tc.id.clone());
                }
            }
        }
    }
    ids
}

/// Look backwards from a tool result to find the tool name.
fn find_tool_name_by_id(messages: &[Message], up_to: usize, tool_call_id: &str) -> String {
    if tool_call_id.is_empty() {
        return String::new();
    }
    for msg in messages[..up_to].iter().rev() {
        if msg.is_ai() {
            for tc in msg.tool_calls() {
                if tc.id == tool_call_id {
                    return tc.name.clone();
                }
            }
        }
    }
    String::new()
}

/// Check if a tool should skip pruning based on allow/deny lists.
fn should_skip_pruning(tool_name: &str, allow: &[String], deny: &[String]) -> bool {
    if tool_name.is_empty() {
        return false;
    }
    // Deny takes precedence — if in deny list, always prune (don't skip)
    if deny.iter().any(|d| tool_name == d || matches_wildcard(d, tool_name)) {
        return false;
    }
    // If allow list is non-empty, only prune tools NOT in the list
    if !allow.is_empty() {
        return allow.iter().any(|a| tool_name == a || matches_wildcard(a, tool_name));
    }
    false
}

/// Simple wildcard matching: supports trailing `*` (e.g. "read_*" matches "read_file").
fn matches_wildcard(pattern: &str, name: &str) -> bool {
    if let Some(prefix) = pattern.strip_suffix('*') {
        name.starts_with(prefix)
    } else {
        pattern == name
    }
}
