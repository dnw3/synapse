use std::path::PathBuf;

use async_trait::async_trait;
use synaptic_core::{MemoryStore, Message, SynapseError};

/// A chat message history backed by a JSON file on disk.
///
/// Messages are stored as a JSON array. Each session gets its own file
/// by appending the session ID to the base path.
pub struct FileChatMessageHistory {
    path: PathBuf,
}

impl FileChatMessageHistory {
    /// Create a new file-backed message history.
    ///
    /// The `path` specifies the base directory where session files are stored.
    /// Each session is stored as `{path}/{session_id}.json`.
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    fn session_path(&self, session_id: &str) -> PathBuf {
        self.path.join(format!("{session_id}.json"))
    }

    async fn read_messages(&self, session_id: &str) -> Result<Vec<Message>, SynapseError> {
        let path = self.session_path(session_id);
        match tokio::fs::read_to_string(&path).await {
            Ok(contents) => {
                let messages: Vec<Message> = serde_json::from_str(&contents).map_err(|e| {
                    SynapseError::Memory(format!("failed to parse {}: {e}", path.display()))
                })?;
                Ok(messages)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Vec::new()),
            Err(e) => Err(SynapseError::Memory(format!(
                "failed to read {}: {e}",
                path.display()
            ))),
        }
    }

    async fn write_messages(
        &self,
        session_id: &str,
        messages: &[Message],
    ) -> Result<(), SynapseError> {
        // Ensure directory exists
        if let Some(parent) = self.session_path(session_id).parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(|e| SynapseError::Memory(format!("failed to create directory: {e}")))?;
        }

        let path = self.session_path(session_id);
        let json = serde_json::to_string_pretty(messages)
            .map_err(|e| SynapseError::Memory(format!("failed to serialize messages: {e}")))?;
        tokio::fs::write(&path, json)
            .await
            .map_err(|e| SynapseError::Memory(format!("failed to write {}: {e}", path.display())))
    }
}

#[async_trait]
impl MemoryStore for FileChatMessageHistory {
    async fn append(&self, session_id: &str, message: Message) -> Result<(), SynapseError> {
        let mut messages = self.read_messages(session_id).await?;
        messages.push(message);
        self.write_messages(session_id, &messages).await
    }

    async fn load(&self, session_id: &str) -> Result<Vec<Message>, SynapseError> {
        self.read_messages(session_id).await
    }

    async fn clear(&self, session_id: &str) -> Result<(), SynapseError> {
        self.write_messages(session_id, &[]).await
    }
}
