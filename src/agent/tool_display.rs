//! Configuration-driven tool display resolution.
//!
//! Maps tool name + args -> ToolDisplayMeta (emoji, label, verb, detail).
//! Built-in defaults cover common tools; unknown tools get auto-labeled
//! with a fallback detailKeys scan.

use std::collections::HashMap;

use serde::Deserialize;
use synaptic::graph::streaming::ToolDisplayMeta;

// ---------------------------------------------------------------------------
// Config types
// ---------------------------------------------------------------------------

/// Per-action display override (for multi-action tools).
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ToolDisplayActionSpec {
    pub label: Option<String>,
    pub detail_keys: Option<Vec<String>>,
}

/// Per-tool display specification.
#[derive(Debug, Clone, Deserialize, Default)]
pub struct ToolDisplaySpec {
    pub emoji: Option<String>,
    pub label: Option<String>,
    pub verb: Option<String>,
    pub detail_keys: Option<Vec<String>>,
    /// Per-action overrides keyed by args["action"] value.
    pub actions: Option<HashMap<String, ToolDisplayActionSpec>>,
}

/// Top-level display configuration.
#[derive(Debug, Clone, Deserialize)]
pub struct ToolDisplayConfig {
    #[serde(default = "default_fallback_emoji")]
    pub fallback_emoji: String,
    #[serde(default = "default_fallback_detail_keys")]
    pub fallback_detail_keys: Vec<String>,
    #[serde(default)]
    pub tools: HashMap<String, ToolDisplaySpec>,
}

fn default_fallback_emoji() -> String {
    "\u{1f9e9}".into() // puzzle piece
}

fn default_fallback_detail_keys() -> Vec<String> {
    [
        "command",
        "path",
        "file_path",
        "url",
        "query",
        "pattern",
        "name",
        "description",
        "skill",
        "key",
        "target",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

impl Default for ToolDisplayConfig {
    fn default() -> Self {
        Self {
            fallback_emoji: default_fallback_emoji(),
            fallback_detail_keys: default_fallback_detail_keys(),
            tools: HashMap::new(),
        }
    }
}

// ---------------------------------------------------------------------------
// Resolver
// ---------------------------------------------------------------------------

pub struct ToolDisplayResolver {
    config: ToolDisplayConfig,
}

impl ToolDisplayResolver {
    pub fn new(mut config: ToolDisplayConfig) -> Self {
        // Merge built-in defaults under user config (user wins)
        for (name, spec) in builtin_tools() {
            config.tools.entry(name).or_insert(spec);
        }
        Self { config }
    }

    /// Resolve display metadata for a tool call.
    pub fn resolve(&self, name: &str, args: &serde_json::Value) -> ToolDisplayMeta {
        let spec = self.config.tools.get(name);

        let emoji = spec
            .and_then(|s| s.emoji.clone())
            .unwrap_or_else(|| self.config.fallback_emoji.clone());

        // Per-action overrides
        let action = args.get("action").and_then(|v| v.as_str());
        let action_spec = action.and_then(|a| spec.and_then(|s| s.actions.as_ref()?.get(a)));

        let label = action_spec
            .and_then(|a| a.label.clone())
            .or_else(|| spec.and_then(|s| s.label.clone()))
            .unwrap_or_else(|| auto_label(name));

        let verb = spec.and_then(|s| s.verb.clone()).unwrap_or_default();

        let detail_keys = action_spec
            .and_then(|a| a.detail_keys.as_ref())
            .or_else(|| spec.and_then(|s| s.detail_keys.as_ref()))
            .unwrap_or(&self.config.fallback_detail_keys);

        let detail = extract_detail(args, detail_keys);

        ToolDisplayMeta {
            emoji,
            label,
            verb,
            detail,
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// "read_file" -> "Read File"
fn auto_label(name: &str) -> String {
    name.split('_')
        .map(|w| {
            let mut c = w.chars();
            match c.next() {
                None => String::new(),
                Some(f) => f.to_uppercase().to_string() + c.as_str(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// Extract detail from args by trying keys in order.
fn extract_detail(args: &serde_json::Value, keys: &[String]) -> String {
    for key in keys {
        if let Some(val) = args.get(key) {
            let s = match val {
                serde_json::Value::String(s) if !s.is_empty() => s.clone(),
                serde_json::Value::Null | serde_json::Value::String(_) => continue,
                other => {
                    let rendered = other.to_string();
                    if rendered.len() > 120 {
                        format!("{}...", &rendered[..117])
                    } else {
                        rendered
                    }
                }
            };
            return shorten_home(&truncate_str(&s, 80));
        }
    }
    String::new()
}

fn truncate_str(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max.saturating_sub(3)])
    }
}

fn shorten_home(s: &str) -> String {
    if let Some(home) = dirs::home_dir() {
        let home_str = home.to_string_lossy();
        if s.starts_with(home_str.as_ref()) {
            return format!("~{}", &s[home_str.len()..]);
        }
    }
    s.to_string()
}

// ---------------------------------------------------------------------------
// Built-in tool display config
// ---------------------------------------------------------------------------

fn spec(emoji: &str, label: &str, verb: &str, keys: &[&str]) -> ToolDisplaySpec {
    ToolDisplaySpec {
        emoji: Some(emoji.into()),
        label: Some(label.into()),
        verb: Some(verb.into()),
        detail_keys: Some(keys.iter().map(|s| s.to_string()).collect()),
        actions: None,
    }
}

fn builtin_tools() -> HashMap<String, ToolDisplaySpec> {
    HashMap::from([
        (
            "execute".into(),
            spec("\u{26a1}", "Execute", "executing", &["command"]),
        ),
        (
            "read_file".into(),
            spec("\u{1f4d6}", "Read", "reading", &["path"]),
        ),
        (
            "write_file".into(),
            spec("\u{270d}\u{fe0f}", "Write", "writing", &["path"]),
        ),
        (
            "edit_file".into(),
            spec("\u{270f}\u{fe0f}", "Edit", "editing", &["path"]),
        ),
        ("ls".into(), spec("\u{1f4c2}", "List", "listing", &["path"])),
        (
            "glob".into(),
            spec("\u{1f50d}", "Glob", "searching", &["pattern"]),
        ),
        (
            "grep".into(),
            spec("\u{1f50e}", "Grep", "searching", &["pattern"]),
        ),
        (
            "task".into(),
            spec("\u{1f916}", "Sub-agent", "spawning", &["description"]),
        ),
        (
            "Skill".into(),
            spec("\u{1f3af}", "Skill", "invoking", &["skill"]),
        ),
        (
            "memory_search".into(),
            spec("\u{1f9e0}", "Memory", "searching", &["query"]),
        ),
        (
            "memory_save".into(),
            spec("\u{1f9e0}", "Memory", "saving", &["key"]),
        ),
    ])
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn resolver() -> ToolDisplayResolver {
        ToolDisplayResolver::new(ToolDisplayConfig::default())
    }

    #[test]
    fn known_tool_resolves() {
        let r = resolver();
        let meta = r.resolve("execute", &json!({"command": "ls -la ~/project"}));
        assert_eq!(meta.emoji, "\u{26a1}");
        assert_eq!(meta.label, "Execute");
        assert!(meta.detail.contains("ls -la"));
    }

    #[test]
    fn unknown_tool_gets_fallback() {
        let r = resolver();
        let meta = r.resolve("my_custom_mcp_tool", &json!({"query": "hello"}));
        assert_eq!(meta.emoji, "\u{1f9e9}");
        assert_eq!(meta.label, "My Custom Mcp Tool");
        assert_eq!(meta.detail, "hello");
    }

    #[test]
    fn unknown_tool_no_matching_keys() {
        let r = resolver();
        let meta = r.resolve("exotic_tool", &json!({"foo": "bar"}));
        assert_eq!(meta.detail, "");
    }

    #[test]
    fn empty_args() {
        let r = resolver();
        let meta = r.resolve("execute", &json!({}));
        assert_eq!(meta.detail, "");
    }

    #[test]
    fn auto_label_formatting() {
        assert_eq!(auto_label("read_file"), "Read File");
        assert_eq!(auto_label("memory_search"), "Memory Search");
        assert_eq!(auto_label("ls"), "Ls");
        assert_eq!(auto_label("Skill"), "Skill");
    }

    #[test]
    fn detail_truncation() {
        let r = resolver();
        let long_cmd = "a".repeat(200);
        let meta = r.resolve("execute", &json!({"command": long_cmd}));
        assert!(meta.detail.len() <= 83);
    }
}
