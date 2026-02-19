use synaptic::core::{MemoryStore, Message, SynapticError};
use synaptic::memory::InMemoryStore;

#[tokio::main]
async fn main() -> Result<(), SynapticError> {
    // --- 1. Basic memory usage ---
    let memory = InMemoryStore::new();
    let session_id = "demo-session";

    // Append a multi-turn conversation
    memory
        .append(session_id, Message::human("Hello, Synaptic"))
        .await?;
    memory
        .append(session_id, Message::ai("Hello! How can I help you today?"))
        .await?;
    memory
        .append(session_id, Message::human("What can you do?"))
        .await?;
    memory
        .append(
            session_id,
            Message::ai("I can answer questions, call tools, and more."),
        )
        .await?;

    // Load and print the full transcript
    println!("=== Session transcript ===");
    let transcript = memory.load(session_id).await?;
    for msg in &transcript {
        println!("  {}: {}", msg.role(), msg.content());
    }
    println!("  ({} messages total)\n", transcript.len());

    // --- 2. Session isolation ---
    // Different sessions are completely independent
    let session_a = "user-alice";
    let session_b = "user-bob";

    memory
        .append(session_a, Message::human("I'm Alice"))
        .await?;
    memory
        .append(session_b, Message::human("I'm Bob"))
        .await?;

    let alice_msgs = memory.load(session_a).await?;
    let bob_msgs = memory.load(session_b).await?;

    println!("=== Session isolation ===");
    println!(
        "  Alice's session: {} message(s) — \"{}\"",
        alice_msgs.len(),
        alice_msgs[0].content()
    );
    println!(
        "  Bob's session: {} message(s) — \"{}\"",
        bob_msgs.len(),
        bob_msgs[0].content()
    );

    // Original session is unaffected
    let original = memory.load(session_id).await?;
    println!(
        "  Original session: {} message(s) (unchanged)\n",
        original.len()
    );

    // --- 3. Clearing a session ---
    memory.clear(session_a).await?;
    let after_clear = memory.load(session_a).await?;
    println!("=== Clear ===");
    println!(
        "  Alice's session after clear: {} message(s)",
        after_clear.len()
    );

    // Bob's session is still intact
    let bob_after = memory.load(session_b).await?;
    println!(
        "  Bob's session still has: {} message(s)",
        bob_after.len()
    );

    Ok(())
}
