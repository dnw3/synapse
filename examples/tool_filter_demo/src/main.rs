use serde_json::json;
use synaptic::core::ToolDefinition;
use synaptic::tools::{AllowListFilter, FilterContext, StateMachineFilter, ToolFilter};

fn make_tool(name: &str) -> ToolDefinition {
    ToolDefinition {
        name: name.to_string(),
        description: format!("{} tool", name),
        parameters: json!({"type": "object"}),
        extras: None,
    }
}

#[tokio::main]
async fn main() {
    println!("=== Tool Filter Demo ===\n");

    let all_tools = vec![
        make_tool("search"),
        make_tool("read_file"),
        make_tool("write_file"),
        make_tool("execute_command"),
        make_tool("delete_file"),
    ];

    // 1. AllowListFilter: only permit safe tools
    println!("--- AllowListFilter (search, read_file) ---");
    let allow = AllowListFilter::new(["search", "read_file"]);
    let ctx = FilterContext::default();
    let filtered = allow.filter(all_tools.clone(), &ctx);
    println!(
        "Available tools: {:?}\n",
        filtered.iter().map(|t| &t.name).collect::<Vec<_>>()
    );

    // 2. StateMachineFilter: after-tool rules
    println!("--- StateMachineFilter: after-tool rules ---");
    let sm = StateMachineFilter::new()
        .after_tool("search", ["read_file", "search"])
        .after_tool("read_file", ["write_file", "search"]);

    // No last tool yet => all tools available
    let ctx_start = FilterContext::default();
    let filtered = sm.filter(all_tools.clone(), &ctx_start);
    println!(
        "Turn 0 (no last tool): {:?}",
        filtered.iter().map(|t| &t.name).collect::<Vec<_>>()
    );

    // After "search" => only read_file and search
    let ctx_after_search = FilterContext {
        last_tool: Some("search".to_string()),
        ..Default::default()
    };
    let filtered = sm.filter(all_tools.clone(), &ctx_after_search);
    println!(
        "After 'search':        {:?}",
        filtered.iter().map(|t| &t.name).collect::<Vec<_>>()
    );

    // After "read_file" => only write_file and search
    let ctx_after_read = FilterContext {
        last_tool: Some("read_file".to_string()),
        ..Default::default()
    };
    let filtered = sm.filter(all_tools.clone(), &ctx_after_read);
    println!(
        "After 'read_file':     {:?}\n",
        filtered.iter().map(|t| &t.name).collect::<Vec<_>>()
    );

    // 3. StateMachineFilter: turn threshold rules
    println!("--- StateMachineFilter: turn thresholds ---");
    let sm_turns = StateMachineFilter::new()
        .turn_threshold(3, ["write_file", "delete_file"])
        .turn_threshold(5, ["execute_command"]);

    for turn in [0, 2, 3, 5] {
        let ctx = FilterContext {
            turn_count: turn,
            ..Default::default()
        };
        let filtered = sm_turns.filter(all_tools.clone(), &ctx);
        println!(
            "Turn {}: {:?}",
            turn,
            filtered.iter().map(|t| &t.name).collect::<Vec<_>>()
        );
    }

    println!("\nDone.");
}
