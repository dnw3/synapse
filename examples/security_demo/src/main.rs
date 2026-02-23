use serde_json::json;
use synaptic::middleware::{RiskLevel, RuleBasedAnalyzer, SecurityAnalyzer};

#[tokio::main]
async fn main() {
    println!("=== Security RuleBasedAnalyzer Demo ===\n");

    // Build an analyzer with tool-level and argument-pattern rules
    let analyzer = RuleBasedAnalyzer::new()
        .with_default_risk(RiskLevel::Low)
        .with_tool_risk("read_file", RiskLevel::Low)
        .with_tool_risk("write_file", RiskLevel::Medium)
        .with_tool_risk("execute_command", RiskLevel::High)
        .with_tool_risk("delete_database", RiskLevel::Critical)
        .with_arg_pattern("path", "/etc/", RiskLevel::Critical)
        .with_arg_pattern("command", "rm ", RiskLevel::Critical);

    // Assess various tool calls
    let cases = vec![
        ("read_file", json!({"path": "/tmp/notes.txt"})),
        (
            "write_file",
            json!({"path": "/home/user/output.txt", "content": "hello"}),
        ),
        ("execute_command", json!({"command": "ls -la"})),
        ("execute_command", json!({"command": "rm -rf /important"})),
        ("delete_database", json!({"db": "production"})),
        ("read_file", json!({"path": "/etc/shadow"})),
        ("unknown_tool", json!({"key": "value"})),
    ];

    for (tool_name, args) in cases {
        let risk = analyzer.assess(tool_name, &args).await.unwrap();
        println!(
            "Tool: {:<20} Args: {:<40} => Risk: {:?}",
            tool_name,
            args.to_string(),
            risk,
        );
    }

    println!("\nNote: The /etc/shadow read_file call was elevated to Critical");
    println!("because the arg_pattern rule matched '/etc/' in the path.");
    println!("\nDone.");
}
