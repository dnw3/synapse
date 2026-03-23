use crate::config::SynapseConfig;

/// Build the self-awareness section for the agent's system prompt.
///
/// This tells the agent about its own identity, configuration, available
/// context files, and how to modify its own behavior at runtime.
pub fn build_self_section(config: &SynapseConfig, channel: &str) -> String {
    let version = env!("CARGO_PKG_VERSION");

    // Only expose internal paths/API info for local channels (web/repl)
    let is_local = matches!(channel, "web" | "repl");

    let config_file = if is_local {
        if std::path::Path::new("synapse.toml").exists() {
            "./synapse.toml"
        } else {
            "synapse.toml (not found)"
        }
    } else {
        "(not available in this channel)"
    };

    let session_dir = if is_local {
        config.sessions_dir()
    } else {
        "(internal)"
    };
    let log_dir = if is_local {
        "~/.synapse/logs/"
    } else {
        "(internal)"
    };

    // Channel capabilities summary
    let (capabilities, msg_limit) = channel_info_for(channel);
    let cap_str = if capabilities.is_empty() {
        String::new()
    } else {
        capabilities.join(", ")
    };
    let limit_str = if msg_limit == 0 {
        "unlimited".to_string()
    } else {
        format!("{}", msg_limit)
    };

    let mut section = format!(
        "\
# Self

- **product**: Synapse v{version}
- **config_file**: {config_file} (requires restart for changes)
- **session_dir**: {session_dir}
- **log_dir**: {log_dir}
- **log_format**: LogID = 23 chars: first 13 digits = millisecond timestamp (decimal), next 6 hex = machine ID, last 4 hex = random
- **channel**: {channel} (capabilities: {cap_str}, message limit: {limit_str})

# Context Files (editable, takes effect on next model call)

| File | Purpose | Example |
|------|---------|---------|
| `IDENTITY.md` | Visual identity: name, emoji, avatar URL, theme color | Change your display name or avatar |
| `SOUL.md` | Core persona: values, tone, style, behavioral boundaries | Adjust personality, add expertise areas |
| `AGENTS.md` | Session instructions: startup behavior, memory rules | Change how you greet users or manage context |
| `MEMORY.md` | Persistent notes: learned preferences, patterns | Store facts to remember across sessions |
| `USER.md` | User profile: name, timezone, preferences, communication style | Adapt to user's working hours or language |
| `TOOLS.md` | Tool usage guidelines and environment description | Add tips for using specific tools |
| `BOOTSTRAP.md` | First-run instructions (runs once) | Initial setup procedures |

# Self-Modification

When the user asks you to change your behavior, persona, or remember something:
1. Identify which context file is most appropriate (see table above)
2. Read the current file content with `read_file`
3. Make targeted edits with `edit_file` (prefer edit over full rewrite)
4. Changes take effect on the next model call — no restart needed

When the user asks about your configuration:
- Read `synapse.toml` for current settings
- Config changes require editing the file AND restarting the server

When the user asks about logs or debugging:
- LogID format: first 13 decimal digits = millisecond timestamp (decode: `new Date(parseInt(id.slice(0,13), 10))`)"
    );

    // Add log/API path info only for local channels
    if is_local {
        section.push_str("\n- Log files: `~/.synapse/logs/synapse.log.YYYY-MM-DD`");
        section.push_str("\n- Query logs: `GET /api/logs?request_id={logid}` or grep log files");
    }

    // Add channel-specific guidance
    match channel {
        "web" => {
            section.push_str("\n\nYou are running in the web gateway. You have access to streaming, canvas rendering, file uploads, and approval dialogs.");
        }
        "repl" => {
            section.push_str("\n\nYou are running in the interactive REPL. The user can use slash commands and session switching.");
        }
        "lark" => {
            section.push_str("\n\nYou are running as a Lark bot. Messages are limited to 4096 chars — use cards for rich content.");
        }
        "slack" => {
            section.push_str(
                "\n\nYou are running as a Slack bot. Messages are limited to 4000 chars.",
            );
        }
        "telegram" => {
            section.push_str(
                "\n\nYou are running as a Telegram bot. Messages are limited to 4096 chars.",
            );
        }
        "discord" => {
            section.push_str(
                "\n\nYou are running as a Discord bot. Messages are limited to 2000 chars.",
            );
        }
        _ => {}
    }

    section
}

/// Return (capabilities, message_char_limit) for a given channel.
pub fn channel_info_for(channel: &str) -> (Vec<&'static str>, usize) {
    match channel {
        "web" => (
            vec![
                "streaming",
                "canvas",
                "file_upload",
                "approval_dialog",
                "rpc",
                "tool_visibility",
                "idempotency",
            ],
            0,
        ),
        "lark" => (
            vec![
                "streaming",
                "cards",
                "threading",
                "multimodal",
                "markdown",
                "file_upload",
            ],
            4096,
        ),
        "slack" => (vec!["reactions", "socket_mode"], 4000),
        "telegram" => (
            vec![
                "typing_indicator",
                "reactions",
                "multimodal",
                "long_polling",
            ],
            4096,
        ),
        "discord" => (vec!["reactions", "multimodal", "gateway"], 2000),
        "dingtalk" => (vec!["webhook", "signature_auth"], 20000),
        "wechat" => (vec!["webhook", "xml_protocol"], 2048),
        "teams" => (vec!["oauth", "activities", "group_detection"], 4000),
        "repl" => (
            vec![
                "streaming",
                "slash_commands",
                "session_switching",
                "cost_tracking",
                "memory_management",
            ],
            0,
        ),
        _ => (vec![], 0),
    }
}

#[allow(dead_code)]
/// Build channel capabilities for the given channel name.
///
/// Returns a vec of `(channel_name, capabilities, message_limit)` tuples.
/// This is a data helper — the actual `ChannelInfo` struct comes from the framework.
pub fn build_channel_capabilities(channel: &str) -> Vec<(&str, Vec<&'static str>, usize)> {
    let all_channels: &[&str] = &[
        "web", "lark", "slack", "telegram", "discord", "dingtalk", "wechat", "teams", "repl",
    ];

    if channel == "all" {
        all_channels
            .iter()
            .map(|ch| {
                let (caps, limit) = channel_info_for(ch);
                (*ch, caps, limit)
            })
            .collect()
    } else {
        let (caps, limit) = channel_info_for(channel);
        vec![(channel, caps, limit)]
    }
}
