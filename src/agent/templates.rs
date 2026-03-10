/// Default workspace template files for project context injection.

pub struct WorkspaceTemplate {
    pub filename: &'static str,
    pub description: &'static str,
    pub category: &'static str,
    pub icon: &'static str,
    pub default_content: &'static str,
}

pub const WORKSPACE_TEMPLATES: &[WorkspaceTemplate] = &[
    WorkspaceTemplate {
        filename: "SOUL.md",
        description: "Core values, tone, style, and behavioral boundaries",
        category: "personality",
        icon: "heart",
        default_content: r#"# Soul

Define your AI assistant's core personality here.

## Values
- Be helpful, accurate, and concise
- Prioritize user safety and privacy
- Be transparent about limitations

## Tone
- Professional yet approachable
- Clear and direct communication
- Adapt formality to context

## Boundaries
- Never fabricate information
- Acknowledge uncertainty
- Respect user preferences
"#,
    },
    WorkspaceTemplate {
        filename: "IDENTITY.md",
        description: "Visual identity: name, emoji, avatar, theme color",
        category: "personality",
        icon: "palette",
        default_content: r#"---
name: Synapse
emoji: ⚡
avatar_url:
theme_color:
---

# Identity

This file defines the visual identity of your AI assistant.
Edit the YAML frontmatter above to customize:

- **name**: Display name shown in the UI header
- **emoji**: Emoji shown as the assistant avatar
- **avatar_url**: URL to a custom avatar image (overrides emoji)
- **theme_color**: CSS color value for the accent color (e.g., #6366f1)
"#,
    },
    WorkspaceTemplate {
        filename: "USER.md",
        description: "User profile: name, timezone, preferences",
        category: "profile",
        icon: "user",
        default_content: r#"# User Profile

Describe yourself so the assistant can personalize responses.

## About
- Name:
- Role:
- Timezone:

## Preferences
- Language: English
- Response style: concise
- Code style: modern, minimal comments

## Context
- Primary project:
- Tech stack:
"#,
    },
    WorkspaceTemplate {
        filename: "AGENTS.md",
        description: "Session startup instructions and memory management",
        category: "session",
        icon: "bot",
        default_content: r#"# Agent Instructions

Instructions loaded at the start of every session.

## Session Rules
- Always greet the user by name if known
- Check for pending tasks from previous sessions
- Summarize context when resuming a conversation

## Memory Management
- Save important decisions and preferences
- Prune outdated information regularly
- Cross-reference with long-term memory
"#,
    },
    WorkspaceTemplate {
        filename: "TOOLS.md",
        description: "Tool environment description and usage guidelines",
        category: "tools",
        icon: "wrench",
        default_content: r#"# Tools

Describe available tools and their usage guidelines.

## Environment
- OS: (auto-detected)
- Shell: (auto-detected)
- Package manager:

## Tool Guidelines
- Prefer non-destructive operations
- Always confirm before deleting files
- Use version control for all changes
"#,
    },
    WorkspaceTemplate {
        filename: "BOOTSTRAP.md",
        description: "First-run instructions (deleted after first session)",
        category: "bootstrap",
        icon: "rocket",
        default_content: r#"# Bootstrap

This file runs once on the first session and is then deleted.

## First Run Tasks
- Introduce yourself and explain capabilities
- Ask the user about their project and preferences
- Set up any required environment variables
- Offer to customize SOUL.md and USER.md
"#,
    },
    WorkspaceTemplate {
        filename: "HEARTBEAT.md",
        description: "Background task scheduling and heartbeat instructions",
        category: "tools",
        icon: "clock",
        default_content: r#"# Heartbeat

Instructions for periodic background tasks.

## Scheduled Tasks
- Check for dependency updates weekly
- Review and summarize recent changes daily
- Clean up temporary files

## Monitoring
- Watch for build failures
- Alert on security advisories
- Track performance metrics
"#,
    },
];

/// Look up a template by filename.
pub fn find_template(filename: &str) -> Option<&'static WorkspaceTemplate> {
    WORKSPACE_TEMPLATES.iter().find(|t| t.filename == filename)
}
