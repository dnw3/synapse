use synapse_core::{MemoryStore, Message, SynapseError};
use synapse_memory::InMemoryStore;

#[tokio::main]
async fn main() -> Result<(), SynapseError> {
    let memory = InMemoryStore::new();
    let session_id = "demo-session";

    memory
        .append(session_id, Message::human("Hello, Synapse"))
        .await?;
    memory
        .append(session_id, Message::ai("Hello, how can I help you?"))
        .await?;

    let transcript = memory.load(session_id).await?;
    for message in transcript {
        println!("{}: {}", message.role(), message.content());
    }

    Ok(())
}
