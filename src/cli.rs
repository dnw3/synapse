use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
#[command(
    name = "synapse",
    version,
    about = "AI Agent powered by Synaptic framework"
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,

    /// Message to send (single-shot mode, backward compat).
    pub message: Option<String>,

    /// Resume an existing session by ID.
    #[arg(long = "session", short = 's', global = true)]
    pub session_id: Option<String>,

    /// Override the model from config.
    #[arg(short = 'm', long = "model", global = true)]
    pub model_override: Option<String>,

    /// Path to config file.
    #[arg(short = 'c', long = "config", global = true)]
    pub config_path: Option<PathBuf>,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    /// Chat with the AI (REPL or single-shot).
    Chat {
        /// Message to send (omit for REPL mode).
        message: Option<String>,
    },

    /// Run a Deep Agent task (file operations, code generation, etc.).
    Task {
        /// Task description.
        message: String,

        /// Working directory for the agent.
        #[arg(long = "cwd")]
        cwd: Option<PathBuf>,
    },

    /// List all sessions.
    Sessions,

    /// Generate a device pairing QR code and setup code.
    #[cfg(feature = "web")]
    Qr {
        /// Only output the setup code (no QR).
        #[arg(long)]
        setup_code_only: bool,
        /// Override the gateway WebSocket URL.
        #[arg(long)]
        url: Option<String>,
    },

    /// Manage paired devices.
    #[cfg(feature = "web")]
    Devices {
        /// Action: list, approve, reject, remove, rename, rotate, revoke.
        action: String,
        /// Request ID (for approve/reject).
        #[arg(long)]
        request_id: Option<String>,
        /// Device ID (for remove/rename/rotate/revoke).
        #[arg(long)]
        device: Option<String>,
        /// New name (for rename).
        #[arg(long)]
        name: Option<String>,
    },

    /// Manage DM pairing for bot channels.
    #[cfg(feature = "web")]
    Pairing {
        /// Action: list, approve, allowlist, remove.
        action: String,
        /// Channel name (e.g., lark, telegram).
        channel: String,
        /// Pairing code (for approve) or sender ID (for remove).
        value: Option<String>,
    },

    /// Interactive setup wizard — creates synapse.toml config file.
    Init,

    /// Health check — verifies config, API keys, model connectivity, and integrations.
    Doctor,

    /// Start the web server.
    #[cfg(feature = "web")]
    Serve {
        /// Host to bind to.
        #[arg(long, default_value = "0.0.0.0")]
        host: Option<String>,

        /// Port to listen on.
        #[arg(long, default_value = "3000")]
        port: Option<u16>,
    },

    /// Start a bot adapter.
    #[cfg(any(
        feature = "bot-lark",
        feature = "bot-slack",
        feature = "bot-telegram",
        feature = "bot-discord",
        feature = "bot-dingtalk",
        feature = "bot-mattermost",
        feature = "bot-matrix",
        feature = "bot-teams",
        feature = "bot-whatsapp",
        feature = "bot-signal",
        feature = "bot-imessage",
        feature = "bot-line",
        feature = "bot-googlechat",
        feature = "bot-wechat",
        feature = "bot-irc",
    ))]
    Bot {
        /// Bot platform: lark, slack, telegram, discord, dingtalk, mattermost, matrix, whatsapp, signal, imessage, googlechat, wechat.
        platform: String,
    },

    /// Manage skills (list, info, install, enable, disable, remove).
    Skill {
        /// Action: list, info, install, enable, disable, remove.
        action: String,
        /// Skill name or path (depends on action).
        name: Option<String>,
    },

    /// Manage plugins (list, install, enable, disable, remove).
    Plugin {
        /// Action: list, install, enable, disable, remove.
        action: String,
        /// Plugin name or path (depends on action).
        name: Option<String>,
    },

    /// Start voice mode.
    Voice,

    /// Start a tunnel to expose the web server to the internet.
    Tunnel {
        /// Tunnel provider: cloudflared, bore, ssh.
        #[arg(long, default_value = "cloudflared")]
        provider: String,

        /// Port to tunnel (default: same as serve port).
        #[arg(long, default_value = "3000")]
        port: u16,

        /// Remote host for SSH tunneling.
        #[arg(long)]
        remote_host: Option<String>,
    },

    /// Generate systemd/launchd service configuration for running Synapse as a daemon.
    InstallService {
        /// Path to config file to embed in the service definition.
        #[arg(long = "service-config")]
        config: Option<String>,
    },

    /// Send a message to a broadcast group.
    #[cfg(feature = "broadcast")]
    Broadcast {
        /// Broadcast group name.
        group: String,
        /// Message to broadcast.
        message: String,
    },

    /// Start the TUI (terminal user interface) chat mode.
    #[cfg(feature = "tui")]
    Tui,

    /// Manage long-term memory (list, search, status, clear).
    Memory {
        /// Action: list, search, status, clear.
        action: String,
        /// Query string (for search action).
        query: Option<String>,
        /// Maximum results (for search/list).
        #[arg(long, short = 'n', default_value = "10")]
        limit: usize,
    },

    /// Connect to a remote Synapse Gateway via WebSocket.
    #[cfg(feature = "web")]
    Connect {
        /// Gateway WebSocket URL (e.g. ws://localhost:3000).
        url: String,
        /// Session/conversation ID (auto-created if omitted).
        #[arg(short, long)]
        session: Option<String>,
        /// Auth token (or set SYNAPSE_GATEWAY_TOKEN env).
        #[arg(short, long)]
        token: Option<String>,
    },

    /// Start ACP (Agent Communication Protocol) server.
    #[cfg(feature = "web")]
    Acp {
        /// Transport mode: stdio or http.
        #[arg(default_value = "stdio")]
        transport: String,
        /// Port for HTTP transport.
        #[arg(short, long, default_value = "3001")]
        port: u16,
    },

    /// Manage model catalog (list, status, aliases).
    Models {
        /// Action: list, status, aliases.
        #[arg(default_value = "list")]
        action: String,
        /// Model name (optional, for future use).
        name: Option<String>,
    },

    /// Manage workflows (list, run, status, approve, reject).
    Workflow {
        /// Action: list, run, status, approve, reject.
        action: String,
        /// Workflow name or resume token (depends on action).
        name: Option<String>,
        /// JSON input data.
        #[arg(long)]
        input: Option<String>,
        /// JSON data to pass with approval.
        #[arg(long)]
        data: Option<String>,
    },
}
