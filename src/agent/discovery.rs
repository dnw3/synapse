//! Agent directory discovery — loads custom agent definitions from `.claude/agents/`.
//!
//! Each `.md` file (or `<name>/AGENT.md` subdirectory) is parsed as a YAML-frontmatter
//! document, similar to SKILL.md, and converted into a [`SubAgentDef`].
//!
//! Search paths (higher priority first):
//! 1. `<cwd>/.claude/agents/` (project-local)
//! 2. `~/.claude/agents/` (personal/global)

use std::path::Path;
use synaptic_deep::SubAgentDef;

/// Discover agent definitions from `.claude/agents/` directories.
///
/// Returns agent defs ordered by discovery (project-local first, then personal).
pub fn discover_agents(cwd: &Path) -> Vec<SubAgentDef> {
    let mut dirs = Vec::new();

    // Project-local agents (higher priority)
    let project_agents = cwd.join(".claude/agents");
    if project_agents.is_dir() {
        dirs.push(project_agents);
    }

    // Personal/global agents
    if let Some(home) = dirs::home_dir() {
        let personal_agents = home.join(".claude/agents");
        if personal_agents.is_dir() {
            dirs.push(personal_agents);
        }
    }

    let mut agents = Vec::new();
    let mut seen = std::collections::HashSet::new();

    for dir in dirs {
        let entries = match std::fs::read_dir(&dir) {
            Ok(e) => e,
            Err(_) => continue,
        };

        for entry in entries.flatten() {
            let path = entry.path();
            let (agent_path, agent_name) = if path.is_file()
                && path.extension().is_some_and(|e| e == "md")
            {
                let name = path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("")
                    .to_string();
                (path.clone(), name)
            } else if path.is_dir() {
                let agent_md = path.join("AGENT.md");
                if agent_md.exists() {
                    let name = path
                        .file_name()
                        .and_then(|s| s.to_str())
                        .unwrap_or("")
                        .to_string();
                    (agent_md, name)
                } else {
                    continue;
                }
            } else {
                continue;
            };

            if agent_name.is_empty() || seen.contains(&agent_name) {
                continue;
            }

            if let Ok(content) = std::fs::read_to_string(&agent_path) {
                if let Some(def) = parse_agent_frontmatter(&content, &agent_name) {
                    seen.insert(agent_name);
                    agents.push(def);
                }
            }
        }
    }

    agents
}

/// Parse YAML frontmatter from an AGENT.md file into a [`SubAgentDef`].
fn parse_agent_frontmatter(content: &str, fallback_name: &str) -> Option<SubAgentDef> {
    let content = content.trim_start_matches('\u{feff}');
    let mut lines = content.lines();

    if lines.next()?.trim() != "---" {
        return Some(SubAgentDef {
            name: fallback_name.to_string(),
            description: String::new(),
            system_prompt: content.to_string(),
            tools: vec![],
            model: None,
            tool_allow: vec![],
            tool_deny: vec![],
            timeout_secs: None,
            max_turns: None,
            tool_profile: None,
            permission_mode: None,
            skills: vec![],
            background: false,
            hooks: None,
            memory: None,
        });
    }

    let mut fm_lines = Vec::new();
    let mut body = String::new();
    let mut in_body = false;

    for line in lines {
        if !in_body {
            if line.trim() == "---" {
                in_body = true;
                continue;
            }
            fm_lines.push(line);
        } else {
            if !body.is_empty() {
                body.push('\n');
            }
            body.push_str(line);
        }
    }

    let mut name = None;
    let mut description = None;
    let mut tool_allow = Vec::new();
    let mut tool_deny = Vec::new();
    let mut timeout_secs = None;
    let mut max_turns = None;
    let mut tool_profile = None;
    let mut permission_mode = None;
    let mut skills = Vec::new();
    let mut background = false;
    let mut hooks: Option<serde_json::Value> = None;
    let mut memory: Option<String> = None;

    for line in &fm_lines {
        let trimmed = line.trim();
        if let Some((key, val)) = trimmed.split_once(':') {
            let key = key.trim();
            let val = val.trim();
            match key {
                "name" => name = Some(val.to_string()),
                "description" => description = Some(val.to_string()),
                "tools" | "tool-allow" | "tool_allow" => {
                    tool_allow = parse_yaml_array(val);
                }
                "disallowedTools" | "tool-deny" | "tool_deny" => {
                    tool_deny = parse_yaml_array(val);
                }
                "timeout-secs" | "timeout_secs" => {
                    timeout_secs = val.parse::<u64>().ok();
                }
                "max-turns" | "max_turns" => {
                    max_turns = val.parse::<usize>().ok();
                }
                "tool-profile" | "tool_profile" => {
                    tool_profile = Some(val.to_string());
                }
                "permission-mode" | "permission_mode" => {
                    permission_mode = Some(val.to_string());
                }
                "skills" => {
                    skills = parse_yaml_array(val);
                }
                "background" => {
                    background = val == "true";
                }
                "hooks" => {
                    if let Ok(v) = serde_json::from_str::<serde_json::Value>(val) {
                        hooks = Some(v);
                    }
                }
                "memory" => {
                    let v = val.trim_matches('"').trim_matches('\'').to_string();
                    if matches!(v.as_str(), "user" | "project" | "local") {
                        memory = Some(v);
                    }
                }
                _ => {}
            }
        }
    }

    Some(SubAgentDef {
        name: name.unwrap_or_else(|| fallback_name.to_string()),
        description: description.unwrap_or_default(),
        system_prompt: body.trim().to_string(),
        tools: vec![],
        model: None,
        tool_allow,
        tool_deny,
        timeout_secs,
        max_turns,
        tool_profile,
        permission_mode,
        skills,
        background,
        hooks,
        memory,
    })
}

/// Parse a simple YAML inline array like `[Bash, write_file]` into a Vec<String>.
fn parse_yaml_array(s: &str) -> Vec<String> {
    let s = s.trim();
    if s.starts_with('[') && s.ends_with(']') {
        s[1..s.len() - 1]
            .split(',')
            .map(|item| item.trim().trim_matches('"').trim_matches('\'').to_string())
            .filter(|item| !item.is_empty())
            .collect()
    } else if s.is_empty() || s == "[]" {
        Vec::new()
    } else {
        vec![s.to_string()]
    }
}
