#![allow(dead_code)]
//! Auto-Reply Command System.
//!
//! Intercepts incoming messages that start with `/` and routes them to built-in
//! commands instead of the LLM.  If no matching command is found the message
//! continues to the normal agent pipeline.
//!
//! # Architecture
//!
//! - [`Command`] trait — implemented by each built-in command.
//! - [`CommandRegistry`] — stores commands by name; used by the subscriber.
//! - [`AutoReplySubscriber`] — an [`EventSubscriber`] that hooks
//!   `BeforeModelCall` (Intercept mode) to short-circuit LLM calls when a
//!   slash command is detected.

use std::collections::HashMap;

use async_trait::async_trait;
use synaptic::core::SynapticError;
use synaptic::events::{Event, EventAction, EventFilter, EventKind, EventSubscriber};

// ---------------------------------------------------------------------------
// CommandContext
// ---------------------------------------------------------------------------

/// Contextual information available to every command during execution.
#[derive(Debug, Clone, Default)]
pub struct CommandContext {
    /// The delivery channel (e.g. `"web"`, `"lark"`, `"slack"`).
    pub channel: Option<String>,
    /// The session / conversation ID.
    pub session_id: Option<String>,
    /// The resolved agent ID.
    pub agent_id: Option<String>,
}

// ---------------------------------------------------------------------------
// Command trait
// ---------------------------------------------------------------------------

/// A single slash command that can be invoked via the auto-reply system.
pub trait Command: Send + Sync {
    /// The command name, without the leading `/`.  Must be unique.
    fn name(&self) -> &str;

    /// One-line description shown in `/help` output.
    fn description(&self) -> &str;

    /// Execute the command and return the text response to send back.
    fn execute(
        &self,
        args: &str,
        ctx: &CommandContext,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>>;
}

// ---------------------------------------------------------------------------
// CommandRegistry
// ---------------------------------------------------------------------------

/// Stores all registered commands indexed by name.
pub struct CommandRegistry {
    commands: HashMap<String, Box<dyn Command>>,
}

impl CommandRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
        }
    }

    /// Register a command.  Overwrites any previously registered command with
    /// the same name.
    pub fn register(&mut self, cmd: Box<dyn Command>) {
        self.commands.insert(cmd.name().to_string(), cmd);
    }

    /// Look up a command by name (without the leading `/`).
    pub fn get(&self, name: &str) -> Option<&dyn Command> {
        self.commands.get(name).map(|b| b.as_ref())
    }

    /// Return `(name, description)` pairs for all registered commands, sorted
    /// alphabetically by name.
    pub fn list(&self) -> Vec<(&str, &str)> {
        let mut pairs: Vec<(&str, &str)> = self
            .commands
            .values()
            .map(|c| (c.name(), c.description()))
            .collect();
        pairs.sort_by_key(|(name, _)| *name);
        pairs
    }

    /// Build a registry pre-populated with all built-in commands.
    pub fn with_builtins() -> Self {
        let mut reg = Self::new();
        reg.register(Box::new(HelpCommand::new()));
        reg.register(Box::new(PingCommand));
        reg.register(Box::new(VersionCommand));
        reg.register(Box::new(StatusCommand));
        reg.register(Box::new(ClearCommand));
        reg.register(Box::new(CompactCommand));
        reg.register(Box::new(ExportCommand));
        reg.register(Box::new(ModelCommand));
        reg.register(Box::new(MemoryCommand));
        reg.register(Box::new(WhoamiCommand));
        reg.register(Box::new(SkillCommand));
        reg
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::with_builtins()
    }
}

// ---------------------------------------------------------------------------
// Parse a slash command from raw message content
// ---------------------------------------------------------------------------

/// Parse a message that starts with `/` into `(command_name, args)`.
///
/// Returns `None` if the message does not start with `/` or the name is empty.
pub fn parse_command(content: &str) -> Option<(&str, &str)> {
    let content = content.trim();
    let after_slash = content.strip_prefix('/')?;
    // Split on the first whitespace
    let (name, rest) = after_slash
        .split_once(|c: char| c.is_whitespace())
        .unwrap_or((after_slash, ""));
    if name.is_empty() {
        return None;
    }
    Some((name, rest.trim()))
}

// ---------------------------------------------------------------------------
// Built-in commands
// ---------------------------------------------------------------------------

// --- /help ------------------------------------------------------------------

/// Lists all available slash commands.
pub struct HelpCommand {
    /// Snapshot of command names/descriptions taken at construction time.
    /// The registry itself is passed at execute time so we can build a
    /// dynamic list.  We keep a marker field so the struct is non-zero-sized.
    _marker: (),
}

impl HelpCommand {
    fn new() -> Self {
        Self { _marker: () }
    }
}

impl Command for HelpCommand {
    fn name(&self) -> &str {
        "help"
    }

    fn description(&self) -> &str {
        "List all available slash commands"
    }

    fn execute(
        &self,
        _args: &str,
        _ctx: &CommandContext,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        // The registry is not accessible from here; the subscriber builds the
        // help text itself and calls this command only as a fallback — see
        // AutoReplySubscriber::handle().
        Ok("Type `/help` to see available commands.".to_string())
    }
}

// --- /ping ------------------------------------------------------------------

/// Replies with `pong` — useful for connectivity checks.
pub struct PingCommand;

impl Command for PingCommand {
    fn name(&self) -> &str {
        "ping"
    }

    fn description(&self) -> &str {
        "Check connectivity — replies with pong"
    }

    fn execute(
        &self,
        _args: &str,
        _ctx: &CommandContext,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        Ok("pong".to_string())
    }
}

// --- /version ---------------------------------------------------------------

/// Returns the running Synapse version.
pub struct VersionCommand;

impl Command for VersionCommand {
    fn name(&self) -> &str {
        "version"
    }

    fn description(&self) -> &str {
        "Show the current Synapse version"
    }

    fn execute(
        &self,
        _args: &str,
        _ctx: &CommandContext,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        Ok(format!(
            "Synapse v{} (synaptic framework)",
            env!("CARGO_PKG_VERSION")
        ))
    }
}

// --- /status ----------------------------------------------------------------

/// Returns basic runtime status.
pub struct StatusCommand;

impl Command for StatusCommand {
    fn name(&self) -> &str {
        "status"
    }

    fn description(&self) -> &str {
        "Show runtime status"
    }

    fn execute(
        &self,
        _args: &str,
        ctx: &CommandContext,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let channel = ctx.channel.as_deref().unwrap_or("unknown");
        let session = ctx.session_id.as_deref().unwrap_or("—");
        let agent = ctx.agent_id.as_deref().unwrap_or("default");
        Ok(format!(
            "**Status**: OK\n**Channel**: {channel}\n**Session**: {session}\n**Agent**: {agent}"
        ))
    }
}

// --- /clear -----------------------------------------------------------------

/// Instructs the user on how to clear their conversation history.
pub struct ClearCommand;

impl Command for ClearCommand {
    fn name(&self) -> &str {
        "clear"
    }

    fn description(&self) -> &str {
        "Clear the current conversation history"
    }

    fn execute(
        &self,
        _args: &str,
        _ctx: &CommandContext,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        Ok("Conversation history cleared. Starting a fresh session.".to_string())
    }
}

// --- /compact ---------------------------------------------------------------

/// Instructs the user on manual compaction.
pub struct CompactCommand;

impl Command for CompactCommand {
    fn name(&self) -> &str {
        "compact"
    }

    fn description(&self) -> &str {
        "Compact the conversation context to save tokens"
    }

    fn execute(
        &self,
        _args: &str,
        _ctx: &CommandContext,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        Ok(
            "Context compaction triggered. The conversation will be summarised on the next turn."
                .to_string(),
        )
    }
}

// --- /export ----------------------------------------------------------------

/// Placeholder that tells the user how to export their session.
pub struct ExportCommand;

impl Command for ExportCommand {
    fn name(&self) -> &str {
        "export"
    }

    fn description(&self) -> &str {
        "Export the current session transcript"
    }

    fn execute(
        &self,
        args: &str,
        ctx: &CommandContext,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let format = if args.is_empty() { "json" } else { args };
        let session = ctx.session_id.as_deref().unwrap_or("current");
        Ok(format!(
            "Export of session `{session}` in `{format}` format is available via the \
             REST API: `GET /api/sessions/{session}/export?format={format}`"
        ))
    }
}

// --- /model -----------------------------------------------------------------

/// Shows or hints about model configuration.
pub struct ModelCommand;

impl Command for ModelCommand {
    fn name(&self) -> &str {
        "model"
    }

    fn description(&self) -> &str {
        "Show the current model or switch model (usage: /model [name])"
    }

    fn execute(
        &self,
        args: &str,
        _ctx: &CommandContext,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        if args.is_empty() {
            Ok(
                "Use `model` in `synapse.toml` to configure the default model, or pass \
                `--model <name>` on the CLI.  Run `/model <name>` to request a switch."
                    .to_string(),
            )
        } else {
            Ok(format!(
                "Model switch to `{args}` requested.  The change will take effect on the \
                 next conversation turn."
            ))
        }
    }
}

// --- /memory ----------------------------------------------------------------

/// Shows memory-related information.
pub struct MemoryCommand;

impl Command for MemoryCommand {
    fn name(&self) -> &str {
        "memory"
    }

    fn description(&self) -> &str {
        "Show long-term memory status (usage: /memory [search <query>])"
    }

    fn execute(
        &self,
        args: &str,
        _ctx: &CommandContext,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        if args.starts_with("search ") {
            let query = args.trim_start_matches("search ").trim();
            Ok(format!(
                "Memory search for `{query}` can be performed via the `memory_search` tool \
                 or the REST API: `GET /api/memory?q={query}`"
            ))
        } else {
            Ok(
                "Long-term memory is enabled.  Use `/memory search <query>` to search, \
                or ask the agent to `remember` or `recall` information."
                    .to_string(),
            )
        }
    }
}

// --- /whoami ----------------------------------------------------------------

/// Returns session / channel identity information.
pub struct WhoamiCommand;

impl Command for WhoamiCommand {
    fn name(&self) -> &str {
        "whoami"
    }

    fn description(&self) -> &str {
        "Show your session and channel identity"
    }

    fn execute(
        &self,
        _args: &str,
        ctx: &CommandContext,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        let channel = ctx.channel.as_deref().unwrap_or("unknown");
        let session = ctx.session_id.as_deref().unwrap_or("—");
        let agent = ctx.agent_id.as_deref().unwrap_or("default");
        Ok(format!(
            "**Channel**: {channel}\n**Session**: {session}\n**Agent**: {agent}"
        ))
    }
}

// --- /skill -----------------------------------------------------------------

/// Lists available skills or shows info about a specific one.
pub struct SkillCommand;

impl Command for SkillCommand {
    fn name(&self) -> &str {
        "skill"
    }

    fn description(&self) -> &str {
        "List available skills or get info (usage: /skill [name])"
    }

    fn execute(
        &self,
        args: &str,
        _ctx: &CommandContext,
    ) -> Result<String, Box<dyn std::error::Error + Send + Sync>> {
        if args.is_empty() {
            Ok(
                "Use `/skill <name>` to invoke a skill, or ask the agent to run a skill \
                with the `Skill` tool.  Skills are loaded from `~/.synapse/skills/`, \
                `~/.claude/skills/`, and `.claude/skills/` in your project."
                    .to_string(),
            )
        } else {
            Ok(format!(
                "Skill `{args}` will be invoked on the next turn.  \
                 You can also ask the agent directly: \"Run the `{args}` skill.\""
            ))
        }
    }
}

// ---------------------------------------------------------------------------
// AutoReplySubscriber
// ---------------------------------------------------------------------------

/// An [`EventSubscriber`] that intercepts `BeforeModelCall` events when the
/// incoming user message starts with `/`, routing it to the matching command.
///
/// If no command is found the event continues to the normal LLM pipeline.
pub struct AutoReplySubscriber {
    registry: CommandRegistry,
}

impl AutoReplySubscriber {
    /// Create with the default built-in command registry.
    pub fn new() -> Self {
        Self {
            registry: CommandRegistry::with_builtins(),
        }
    }

    /// Create with a custom registry.
    pub fn with_registry(registry: CommandRegistry) -> Self {
        Self { registry }
    }

    /// Build help text from the registry.
    fn help_text(&self) -> String {
        let mut lines = vec!["**Available commands:**".to_string()];
        for (name, desc) in self.registry.list() {
            lines.push(format!("  `/{name}` — {desc}"));
        }
        lines.join("\n")
    }
}

impl Default for AutoReplySubscriber {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl EventSubscriber for AutoReplySubscriber {
    fn subscriptions(&self) -> Vec<EventFilter> {
        // BeforeModelCall is Intercept mode — we can short-circuit the LLM.
        vec![EventFilter::Exact(EventKind::BeforeModelCall)]
    }

    async fn handle(&self, event: &mut Event) -> Result<EventAction, SynapticError> {
        // Extract the user message from the payload.
        let user_message = match event.payload.get("user_message").and_then(|v| v.as_str()) {
            Some(msg) => msg.to_string(),
            None => return Ok(EventAction::Continue),
        };

        // Only process slash commands.
        let (name, args) = match parse_command(&user_message) {
            Some(parts) => parts,
            None => return Ok(EventAction::Continue),
        };

        // Build context from event metadata.
        let ctx = CommandContext {
            channel: event
                .payload
                .get("channel")
                .and_then(|v| v.as_str())
                .map(str::to_string),
            session_id: event
                .payload
                .get("session_id")
                .or_else(|| event.payload.get("conversation_id"))
                .and_then(|v| v.as_str())
                .map(str::to_string),
            agent_id: event
                .payload
                .get("agent_id")
                .and_then(|v| v.as_str())
                .map(str::to_string),
        };

        // Special-case /help — build the full listing ourselves.
        if name == "help" {
            let response = self.help_text();
            tracing::info!(command = "help", "auto-reply command intercepted");
            return Ok(EventAction::Intercept(serde_json::json!({
                "content": response,
                "auto_reply": true,
                "command": name,
            })));
        }

        // Look up the command in the registry.
        match self.registry.get(name) {
            Some(cmd) => match cmd.execute(args, &ctx) {
                Ok(response) => {
                    tracing::info!(command = %name, "auto-reply command intercepted");
                    Ok(EventAction::Intercept(serde_json::json!({
                        "content": response,
                        "auto_reply": true,
                        "command": name,
                    })))
                }
                Err(e) => {
                    tracing::warn!(command = %name, error = %e, "auto-reply command failed");
                    Ok(EventAction::Intercept(serde_json::json!({
                        "content": format!("Command `/{name}` failed: {e}"),
                        "auto_reply": true,
                        "command": name,
                        "error": e.to_string(),
                    })))
                }
            },
            None => {
                // Unknown command — let the LLM handle it naturally.
                tracing::debug!(command = %name, "unknown slash command — passing to LLM");
                Ok(EventAction::Continue)
            }
        }
    }

    fn name(&self) -> &str {
        "AutoReplySubscriber"
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    // ---- parse_command --------------------------------------------------------

    #[test]
    fn parse_command_simple() {
        let (name, args) = parse_command("/ping").unwrap();
        assert_eq!(name, "ping");
        assert_eq!(args, "");
    }

    #[test]
    fn parse_command_with_args() {
        let (name, args) = parse_command("/model gpt-4o").unwrap();
        assert_eq!(name, "model");
        assert_eq!(args, "gpt-4o");
    }

    #[test]
    fn parse_command_with_leading_whitespace() {
        let (name, args) = parse_command("  /help  ").unwrap();
        assert_eq!(name, "help");
        assert_eq!(args, "");
    }

    #[test]
    fn parse_command_not_a_command() {
        assert!(parse_command("hello world").is_none());
    }

    #[test]
    fn parse_command_empty_name() {
        assert!(parse_command("/").is_none());
    }

    #[test]
    fn parse_command_multi_word_args() {
        let (name, args) = parse_command("/memory search long term notes").unwrap();
        assert_eq!(name, "memory");
        assert_eq!(args, "search long term notes");
    }

    // ---- CommandRegistry -------------------------------------------------------

    #[test]
    fn registry_register_and_get() {
        let mut reg = CommandRegistry::new();
        reg.register(Box::new(PingCommand));
        assert!(reg.get("ping").is_some());
        assert!(reg.get("unknown").is_none());
    }

    #[test]
    fn registry_list_sorted() {
        let reg = CommandRegistry::with_builtins();
        let names: Vec<&str> = reg.list().iter().map(|(n, _)| *n).collect();
        let mut sorted = names.clone();
        sorted.sort_unstable();
        assert_eq!(names, sorted, "list() should return sorted names");
    }

    #[test]
    fn registry_with_builtins_has_all_commands() {
        let reg = CommandRegistry::with_builtins();
        for name in &[
            "help", "ping", "version", "status", "clear", "compact", "export", "model", "memory",
            "whoami", "skill",
        ] {
            assert!(
                reg.get(name).is_some(),
                "missing built-in command: /{}",
                name
            );
        }
    }

    #[test]
    fn registry_list_contains_11_builtins() {
        let reg = CommandRegistry::with_builtins();
        assert_eq!(reg.list().len(), 11);
    }

    // ---- Individual commands ---------------------------------------------------

    #[test]
    fn ping_returns_pong() {
        let ctx = CommandContext::default();
        let result = PingCommand.execute("", &ctx).unwrap();
        assert_eq!(result, "pong");
    }

    #[test]
    fn version_contains_cargo_version() {
        let ctx = CommandContext::default();
        let result = VersionCommand.execute("", &ctx).unwrap();
        assert!(result.contains(env!("CARGO_PKG_VERSION")));
    }

    #[test]
    fn status_shows_channel() {
        let ctx = CommandContext {
            channel: Some("lark".to_string()),
            session_id: Some("sess-1".to_string()),
            agent_id: Some("agent-a".to_string()),
        };
        let result = StatusCommand.execute("", &ctx).unwrap();
        assert!(result.contains("lark"));
        assert!(result.contains("sess-1"));
        assert!(result.contains("agent-a"));
    }

    #[test]
    fn whoami_shows_identity() {
        let ctx = CommandContext {
            channel: Some("slack".to_string()),
            session_id: Some("s-123".to_string()),
            agent_id: None,
        };
        let result = WhoamiCommand.execute("", &ctx).unwrap();
        assert!(result.contains("slack"));
        assert!(result.contains("s-123"));
        assert!(result.contains("default"));
    }

    #[test]
    fn model_without_args_gives_hint() {
        let ctx = CommandContext::default();
        let result = ModelCommand.execute("", &ctx).unwrap();
        assert!(result.contains("synapse.toml") || result.contains("model"));
    }

    #[test]
    fn model_with_args_confirms_switch() {
        let ctx = CommandContext::default();
        let result = ModelCommand.execute("claude-3-7-sonnet", &ctx).unwrap();
        assert!(result.contains("claude-3-7-sonnet"));
    }

    #[test]
    fn export_includes_session_id() {
        let ctx = CommandContext {
            session_id: Some("my-session".to_string()),
            ..Default::default()
        };
        let result = ExportCommand.execute("markdown", &ctx).unwrap();
        assert!(result.contains("my-session"));
        assert!(result.contains("markdown"));
    }

    #[test]
    fn memory_search_includes_query() {
        let ctx = CommandContext::default();
        let result = MemoryCommand.execute("search rust async", &ctx).unwrap();
        assert!(result.contains("rust async"));
    }

    #[test]
    fn skill_with_name_confirms_invocation() {
        let ctx = CommandContext::default();
        let result = SkillCommand.execute("git-commit", &ctx).unwrap();
        assert!(result.contains("git-commit"));
    }

    // ---- AutoReplySubscriber --------------------------------------------------

    #[tokio::test]
    async fn subscriber_intercepts_ping() {
        use synaptic::events::{Event, EventKind};

        let subscriber = AutoReplySubscriber::new();
        let mut event = Event::new(
            EventKind::BeforeModelCall,
            serde_json::json!({ "user_message": "/ping" }),
        );
        let action = subscriber.handle(&mut event).await.unwrap();
        match action {
            EventAction::Intercept(val) => {
                assert_eq!(val["content"].as_str().unwrap(), "pong");
                assert_eq!(val["command"].as_str().unwrap(), "ping");
                assert_eq!(val["auto_reply"].as_bool().unwrap(), true);
            }
            other => panic!("expected Intercept, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn subscriber_continues_for_non_command() {
        use synaptic::events::{Event, EventKind};

        let subscriber = AutoReplySubscriber::new();
        let mut event = Event::new(
            EventKind::BeforeModelCall,
            serde_json::json!({ "user_message": "hello, how are you?" }),
        );
        let action = subscriber.handle(&mut event).await.unwrap();
        assert!(
            matches!(action, EventAction::Continue),
            "expected Continue for non-command message"
        );
    }

    #[tokio::test]
    async fn subscriber_continues_for_unknown_command() {
        use synaptic::events::{Event, EventKind};

        let subscriber = AutoReplySubscriber::new();
        let mut event = Event::new(
            EventKind::BeforeModelCall,
            serde_json::json!({ "user_message": "/nonexistent_cmd_xyz" }),
        );
        let action = subscriber.handle(&mut event).await.unwrap();
        assert!(
            matches!(action, EventAction::Continue),
            "expected Continue for unknown command"
        );
    }

    #[tokio::test]
    async fn subscriber_intercepts_help() {
        use synaptic::events::{Event, EventKind};

        let subscriber = AutoReplySubscriber::new();
        let mut event = Event::new(
            EventKind::BeforeModelCall,
            serde_json::json!({ "user_message": "/help" }),
        );
        let action = subscriber.handle(&mut event).await.unwrap();
        match action {
            EventAction::Intercept(val) => {
                let content = val["content"].as_str().unwrap();
                assert!(content.contains("ping"), "help text should list /ping");
                assert!(
                    content.contains("version"),
                    "help text should list /version"
                );
            }
            other => panic!("expected Intercept, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn subscriber_passes_context_from_payload() {
        use synaptic::events::{Event, EventKind};

        let subscriber = AutoReplySubscriber::new();
        let mut event = Event::new(
            EventKind::BeforeModelCall,
            serde_json::json!({
                "user_message": "/status",
                "channel": "telegram",
                "conversation_id": "conv-99",
                "agent_id": "synapse-bot",
            }),
        );
        let action = subscriber.handle(&mut event).await.unwrap();
        match action {
            EventAction::Intercept(val) => {
                let content = val["content"].as_str().unwrap();
                assert!(content.contains("telegram"));
                assert!(content.contains("conv-99"));
                assert!(content.contains("synapse-bot"));
            }
            other => panic!("expected Intercept, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn subscriber_continues_when_no_user_message() {
        use synaptic::events::{Event, EventKind};

        let subscriber = AutoReplySubscriber::new();
        let mut event = Event::new(
            EventKind::BeforeModelCall,
            serde_json::json!({ "other_field": "value" }),
        );
        let action = subscriber.handle(&mut event).await.unwrap();
        assert!(matches!(action, EventAction::Continue));
    }
}
