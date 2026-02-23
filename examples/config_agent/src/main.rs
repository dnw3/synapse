use std::path::Path;

use synaptic::config::SynapticAgentConfig;

#[tokio::main]
async fn main() {
    println!("=== Config Agent Demo ===\n");

    // Load from the example's synaptic.toml
    let config_path = Path::new(env!("CARGO_MANIFEST_DIR")).join("synaptic.toml");
    println!("Loading config from: {}\n", config_path.display());

    let config = match SynapticAgentConfig::load(Some(&config_path)) {
        Ok(c) => c,
        Err(e) => {
            println!("Failed to load config: {}", e);
            return;
        }
    };

    // Print parsed model configuration
    println!("--- Model Config ---");
    println!("  Provider:    {}", config.model.provider);
    println!("  Model:       {}", config.model.model);
    println!("  API key env: {}", config.model.api_key_env);
    println!("  Max tokens:  {:?}", config.model.max_tokens);
    println!("  Temperature: {:?}", config.model.temperature);

    // Print agent configuration
    println!("\n--- Agent Config ---");
    println!("  System prompt: {:?}", config.agent.system_prompt);
    println!("  Max turns:     {:?}", config.agent.max_turns);
    println!("  Filesystem:    {}", config.agent.tools.filesystem);
    println!("  Sandbox root:  {:?}", config.agent.tools.sandbox_root);

    // Print paths configuration
    println!("\n--- Paths Config ---");
    println!("  Sessions dir: {}", config.paths.sessions_dir);
    println!("  Memory file:  {}", config.paths.memory_file);
    println!("  Skills dir:   {}", config.paths.skills_dir);

    // Print MCP configuration
    println!("\n--- MCP Config ---");
    match &config.mcp {
        Some(servers) => {
            for s in servers {
                println!("  Server: {} ({})", s.name, s.transport);
            }
        }
        None => println!("  (no MCP servers configured)"),
    }

    // Try resolving the API key (will fail if env var not set, which is expected)
    println!("\n--- API Key Resolution ---");
    match config.resolve_api_key() {
        Ok(_) => println!("  API key resolved successfully (not printed for security)."),
        Err(e) => println!("  {} (expected if env var not set)", e),
    }

    println!("\nDone.");
}
