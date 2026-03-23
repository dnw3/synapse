use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use synaptic::core::{RunContext, SynapticError};
use synaptic::middleware::{Interceptor, ModelCaller, ModelRequest, ModelResponse};

use crate::config::ToolPolicyConfig;

// ---------------------------------------------------------------------------
// Built-in tool groups
// ---------------------------------------------------------------------------

/// Returns the built-in tool group definitions.
fn builtin_groups() -> HashMap<String, Vec<String>> {
    let mut m = HashMap::new();
    m.insert(
        "@coding".into(),
        vec![
            "read_file".into(),
            "write_file".into(),
            "edit_file".into(),
            "execute".into(),
            "grep".into(),
            "glob".into(),
            "list_dir".into(),
            "task".into(),
            "apply_patch".into(),
        ],
    );
    m.insert(
        "@web".into(),
        vec![
            "browser_navigate".into(),
            "browser_click".into(),
            "browser_type".into(),
            "browser_snapshot".into(),
            "browser_screenshot".into(),
            "firecrawl".into(),
            "fetch_url".into(),
        ],
    );
    m.insert(
        "@messaging".into(),
        vec![
            "sessions_list".into(),
            "sessions_history".into(),
            "sessions_send".into(),
            "sessions_spawn".into(),
            "memory_search".into(),
            "memory_get".into(),
        ],
    );
    m.insert(
        "@readonly".into(),
        vec![
            "read_file".into(),
            "grep".into(),
            "glob".into(),
            "list_dir".into(),
            "read_pdf".into(),
        ],
    );
    m
}

// ---------------------------------------------------------------------------
// Group expansion
// ---------------------------------------------------------------------------

/// Expand `@group` references in a list of tool names.
///
/// Items that start with `@` are looked up in the merged group map (custom
/// groups override built-ins).  Non-group items are passed through as-is.
pub fn expand_tool_groups(
    list: &[String],
    custom_groups: &HashMap<String, Vec<String>>,
) -> Vec<String> {
    let builtins = builtin_groups();
    let mut result = Vec::new();
    for item in list {
        if item.starts_with('@') {
            // Custom groups take precedence over built-ins.
            if let Some(tools) = custom_groups.get(item).or_else(|| builtins.get(item)) {
                result.extend(tools.iter().cloned());
            } else {
                // Unknown group — keep as-is so the caller can notice.
                result.push(item.clone());
            }
        } else {
            result.push(item.clone());
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Tool name matching
// ---------------------------------------------------------------------------

/// Simple glob-style matching: supports trailing `*` (prefix match) and
/// leading `*` (suffix match).  Otherwise exact match.
fn tool_matches(pattern: &str, name: &str) -> bool {
    if pattern == "*" {
        return true;
    }
    if let Some(prefix) = pattern.strip_suffix('*') {
        return name.starts_with(prefix);
    }
    if let Some(suffix) = pattern.strip_prefix('*') {
        return name.ends_with(suffix);
    }
    pattern == name
}

// ---------------------------------------------------------------------------
// Interceptor
// ---------------------------------------------------------------------------

/// Interceptor that enforces tool-level policies:
///
/// 1. **Owner-only tools** — after the model responds, if any tool call targets
///    an owner-only tool and the conversation doesn't belong to the owner, the
///    response is replaced with an error message.
///
/// 2. **Tool filtering** — before each model call, removes tool definitions
///    that are not in the allow list (if set) or are in the deny list.
pub struct ToolPolicyMiddleware {
    config: Arc<ToolPolicyConfig>,
}

impl ToolPolicyMiddleware {
    pub fn new(config: ToolPolicyConfig) -> Self {
        Self {
            config: Arc::new(config),
        }
    }

    /// Check whether a tool name is owner-only.
    fn is_owner_only(&self, tool_name: &str) -> bool {
        let expanded = expand_tool_groups(&self.config.owner_only_tools, &self.config.tool_groups);
        expanded.iter().any(|pat| tool_matches(pat, tool_name))
    }

    /// Check whether a tool name is allowed given the allow/deny lists.
    fn is_tool_allowed(&self, tool_name: &str) -> bool {
        // If there's an explicit allow list, only tools matching it are allowed.
        if !self.config.tool_allow.is_empty() {
            let allowed = expand_tool_groups(&self.config.tool_allow, &self.config.tool_groups);
            if !allowed.iter().any(|pat| tool_matches(pat, tool_name)) {
                return false;
            }
        }

        // If there's a deny list, matching tools are removed.
        if !self.config.tool_deny.is_empty() {
            let denied = expand_tool_groups(&self.config.tool_deny, &self.config.tool_groups);
            if denied.iter().any(|pat| tool_matches(pat, tool_name)) {
                return false;
            }
        }

        true
    }
}

#[async_trait]
impl Interceptor for ToolPolicyMiddleware {
    async fn wrap_model_call(
        &self,
        mut request: ModelRequest,
        ctx: &RunContext,
        next: &dyn ModelCaller,
    ) -> Result<ModelResponse, SynapticError> {
        // Before: filter tool definitions
        let has_filters = !self.config.tool_allow.is_empty() || !self.config.tool_deny.is_empty();
        if has_filters {
            request.tools.retain(|td| self.is_tool_allowed(&td.name));
        }

        let mut response = next.call(request, ctx).await?;

        // After: check for owner-only tool violations
        if !self.config.owner_only_tools.is_empty() {
            let tool_calls = response.message.tool_calls();
            if !tool_calls.is_empty() {
                let violations: Vec<&str> = tool_calls
                    .iter()
                    .filter(|tc| self.is_owner_only(&tc.name))
                    .map(|tc| tc.name.as_str())
                    .collect();

                if !violations.is_empty() {
                    use synaptic::core::Message;
                    response.message = Message::ai(format!(
                        "I cannot execute the following owner-only tool(s): {}. \
                         This operation requires owner privileges.",
                        violations.join(", ")
                    ));
                }
            }
        }

        Ok(response)
    }
}

// ---------------------------------------------------------------------------
// Helper
// ---------------------------------------------------------------------------

/// Check if a sender ID matches the configured owner.
///
/// Returns `true` if no owners are configured (open access) or if the
/// sender is in the owners list.
#[allow(dead_code)]
pub fn is_owner_sender(sender_id: &str, config: &crate::config::SynapseConfig) -> bool {
    let owners = &config.tool_policy.owners;
    owners.is_empty() || owners.iter().any(|o| o == sender_id)
}

// ---------------------------------------------------------------------------
// Sandbox tool restrictions
// ---------------------------------------------------------------------------

/// Returns tool names that should be denied based on sandbox workspace access.
#[cfg(feature = "sandbox")]
#[allow(dead_code)]
pub fn sandbox_tool_restrictions(access: &synaptic::deep::sandbox::WorkspaceAccess) -> Vec<String> {
    use synaptic::deep::sandbox::WorkspaceAccess;
    match access {
        WorkspaceAccess::ReadOnly => vec!["write_file", "edit_file", "apply_patch"]
            .into_iter()
            .map(String::from)
            .collect(),
        WorkspaceAccess::None => vec![
            "write_file",
            "edit_file",
            "apply_patch",
            "read_file",
            "list_dir",
            "glob",
            "grep",
        ]
        .into_iter()
        .map(String::from)
        .collect(),
        WorkspaceAccess::ReadWrite => vec![],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_expand_tool_groups() {
        let custom = HashMap::new();
        let input = vec!["@readonly".into(), "custom_tool".into()];
        let expanded = expand_tool_groups(&input, &custom);
        assert!(expanded.contains(&"read_file".to_string()));
        assert!(expanded.contains(&"grep".to_string()));
        assert!(expanded.contains(&"custom_tool".to_string()));
    }

    #[test]
    fn test_expand_custom_group_overrides_builtin() {
        let mut custom = HashMap::new();
        custom.insert("@readonly".into(), vec!["only_read".into()]);
        let input = vec!["@readonly".into()];
        let expanded = expand_tool_groups(&input, &custom);
        assert_eq!(expanded, vec!["only_read".to_string()]);
    }

    #[test]
    fn test_tool_matches_exact() {
        assert!(tool_matches("read_file", "read_file"));
        assert!(!tool_matches("read_file", "write_file"));
    }

    #[test]
    fn test_tool_matches_prefix_glob() {
        assert!(tool_matches("browser_*", "browser_navigate"));
        assert!(tool_matches("browser_*", "browser_click"));
        assert!(!tool_matches("browser_*", "read_file"));
    }

    #[test]
    fn test_tool_matches_suffix_glob() {
        assert!(tool_matches("*_file", "read_file"));
        assert!(tool_matches("*_file", "write_file"));
        assert!(!tool_matches("*_file", "execute"));
    }

    #[test]
    fn test_tool_matches_star() {
        assert!(tool_matches("*", "anything"));
    }

    #[test]
    fn test_is_owner_sender_no_owners() {
        let config = crate::config::SynapseConfig::default();
        assert!(is_owner_sender("anyone", &config));
    }
}
