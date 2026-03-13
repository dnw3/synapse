//! Policy evaluation for exec approval commands.

use super::config::{AskPolicy, ExecApprovalsConfig, SecurityMode};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyResult {
    Allow,
    Deny,
    Ask,
}

/// Evaluate a command against the exec approvals config.
pub fn evaluate(
    config: &ExecApprovalsConfig,
    command: &str,
    node_id: Option<&str>,
) -> PolicyResult {
    // Check for node-specific overrides
    let (mode, ask, allowlist) = if let Some(nid) = node_id {
        if let Some(ovr) = config.node_overrides.get(nid) {
            (
                ovr.mode.as_ref().unwrap_or(&config.mode),
                ovr.ask.as_ref().unwrap_or(&config.ask),
                ovr.allowlist.as_ref().unwrap_or(&config.allowlist),
            )
        } else {
            (&config.mode, &config.ask, &config.allowlist)
        }
    } else {
        (&config.mode, &config.ask, &config.allowlist)
    };

    match mode {
        SecurityMode::Deny => PolicyResult::Deny,
        SecurityMode::Full => {
            if *ask == AskPolicy::Always {
                PolicyResult::Ask
            } else {
                PolicyResult::Allow
            }
        }
        SecurityMode::Allowlist => {
            let base_cmd = command.split_whitespace().next().unwrap_or(command);
            let matched = allowlist.iter().any(|pattern| {
                if pattern.contains('*') {
                    glob_match(pattern, base_cmd)
                } else {
                    pattern == base_cmd
                }
            });

            if matched {
                if *ask == AskPolicy::Always {
                    PolicyResult::Ask
                } else {
                    PolicyResult::Allow
                }
            } else if *ask == AskPolicy::Off {
                PolicyResult::Deny
            } else {
                PolicyResult::Ask
            }
        }
    }
}

/// Simple glob matching (supports * wildcard).
fn glob_match(pattern: &str, text: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return pattern == text;
    }

    let mut pos = 0;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if let Some(found) = text[pos..].find(part) {
            if i == 0 && found != 0 {
                return false;
            }
            pos += found + part.len();
        } else {
            return false;
        }
    }

    // If pattern doesn't end with *, the text must end exactly
    if !pattern.ends_with('*') && pos != text.len() {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glob_match() {
        assert!(glob_match("ls", "ls"));
        assert!(!glob_match("ls", "lsof"));
        assert!(glob_match("ls*", "lsof"));
        assert!(glob_match("*cat*", "mycat"));
        assert!(glob_match("git*", "git"));
        assert!(glob_match("git*", "git-log"));
    }

    #[test]
    fn test_policy_deny_mode() {
        let config = ExecApprovalsConfig {
            mode: SecurityMode::Deny,
            ..Default::default()
        };
        assert_eq!(evaluate(&config, "ls", None), PolicyResult::Deny);
    }

    #[test]
    fn test_policy_full_mode() {
        let config = ExecApprovalsConfig {
            mode: SecurityMode::Full,
            ask: AskPolicy::Off,
            ..Default::default()
        };
        assert_eq!(evaluate(&config, "rm -rf /", None), PolicyResult::Allow);
    }

    #[test]
    fn test_policy_allowlist() {
        let config = ExecApprovalsConfig::default();
        assert_eq!(evaluate(&config, "ls", None), PolicyResult::Allow);
        assert_eq!(evaluate(&config, "rm", None), PolicyResult::Ask);
    }
}
