use std::fmt;

/// Typed error for the synapse business layer.
#[derive(Debug)]
pub enum SynapseError {
    Config(String),
    Agent(String),
    Model(String),
    Session(String),
    Gateway(String),
    Channel(String),
    Hub(String),
    Tool(String),
    Plugin(String),
    Skill(String),
    Io(String),
    Internal(String),
}

impl fmt::Display for SynapseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Config(msg) => write!(f, "config error: {msg}"),
            Self::Agent(msg) => write!(f, "agent error: {msg}"),
            Self::Model(msg) => write!(f, "model error: {msg}"),
            Self::Session(msg) => write!(f, "session error: {msg}"),
            Self::Gateway(msg) => write!(f, "gateway error: {msg}"),
            Self::Channel(msg) => write!(f, "channel error: {msg}"),
            Self::Hub(msg) => write!(f, "hub error: {msg}"),
            Self::Tool(msg) => write!(f, "tool error: {msg}"),
            Self::Plugin(msg) => write!(f, "plugin error: {msg}"),
            Self::Skill(msg) => write!(f, "skill error: {msg}"),
            Self::Io(msg) => write!(f, "io error: {msg}"),
            Self::Internal(msg) => write!(f, "internal error: {msg}"),
        }
    }
}

impl std::error::Error for SynapseError {}

/// Convenience alias.
pub type Result<T> = std::result::Result<T, SynapseError>;

impl From<synaptic::core::SynapticError> for SynapseError {
    fn from(e: synaptic::core::SynapticError) -> Self {
        Self::Internal(e.to_string())
    }
}

impl From<std::io::Error> for SynapseError {
    fn from(e: std::io::Error) -> Self {
        Self::Io(e.to_string())
    }
}

impl From<serde_json::Error> for SynapseError {
    fn from(e: serde_json::Error) -> Self {
        Self::Config(e.to_string())
    }
}

impl From<Box<dyn std::error::Error>> for SynapseError {
    fn from(e: Box<dyn std::error::Error>) -> Self {
        Self::Internal(e.to_string())
    }
}

impl From<Box<dyn std::error::Error + Send + Sync>> for SynapseError {
    fn from(e: Box<dyn std::error::Error + Send + Sync>) -> Self {
        Self::Internal(e.to_string())
    }
}

impl From<reqwest::Error> for SynapseError {
    fn from(e: reqwest::Error) -> Self {
        Self::Internal(e.to_string())
    }
}

#[cfg(any(
    feature = "bot-slack",
    feature = "bot-discord",
    feature = "bot-mattermost",
    feature = "bot-whatsapp",
    feature = "web",
))]
impl From<tokio_tungstenite::tungstenite::Error> for SynapseError {
    fn from(e: tokio_tungstenite::tungstenite::Error) -> Self {
        Self::Channel(e.to_string())
    }
}

impl From<String> for SynapseError {
    fn from(s: String) -> Self {
        Self::Internal(s)
    }
}

impl From<&str> for SynapseError {
    fn from(s: &str) -> Self {
        Self::Internal(s.to_string())
    }
}

/// Convert any `Display` error into a `SynapseError::Internal`.
///
/// This is useful for `?` on third-party error types that don't have
/// explicit `From` impls. Use `.map_err(SynapseError::internal)` at
/// call sites.
impl SynapseError {
    pub fn internal(e: impl std::fmt::Display) -> Self {
        Self::Internal(e.to_string())
    }
}
