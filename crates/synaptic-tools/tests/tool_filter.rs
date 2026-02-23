use serde_json::json;
use synaptic_core::ToolDefinition;
use synaptic_tools::{
    AllowListFilter, CompositeFilter, DenyListFilter, FilterContext, StateMachineFilter, ToolFilter,
};

fn make_tool(name: &str) -> ToolDefinition {
    ToolDefinition {
        name: name.to_string(),
        description: format!("{name} tool"),
        parameters: json!({"type": "object", "properties": {}}),
        extras: None,
    }
}

fn tool_names(tools: &[ToolDefinition]) -> Vec<String> {
    tools.iter().map(|t| t.name.clone()).collect()
}

#[test]
fn allow_keeps_listed() {
    let filter = AllowListFilter::new(["read", "write"]);
    let tools = vec![make_tool("read"), make_tool("write"), make_tool("delete")];
    let ctx = FilterContext::default();

    let result = filter.filter(tools, &ctx);
    let names = tool_names(&result);
    assert_eq!(names, vec!["read", "write"]);
}

#[test]
fn deny_removes() {
    let filter = DenyListFilter::new(["delete"]);
    let tools = vec![make_tool("read"), make_tool("write"), make_tool("delete")];
    let ctx = FilterContext::default();

    let result = filter.filter(tools, &ctx);
    let names = tool_names(&result);
    assert_eq!(names, vec!["read", "write"]);
}

#[test]
fn state_machine_after_tool() {
    let filter = StateMachineFilter::new().after_tool("search", ["read", "summarize"]);
    let tools = vec![
        make_tool("search"),
        make_tool("read"),
        make_tool("summarize"),
        make_tool("delete"),
    ];

    let ctx = FilterContext {
        last_tool: Some("search".to_string()),
        ..Default::default()
    };

    let result = filter.filter(tools, &ctx);
    let names = tool_names(&result);
    assert_eq!(names, vec!["read", "summarize"]);
}

#[test]
fn state_machine_no_rule_for_last_tool() {
    let filter = StateMachineFilter::new().after_tool("search", ["read"]);
    let tools = vec![make_tool("search"), make_tool("read"), make_tool("delete")];

    // last_tool is "write" which has no rule — all tools pass through
    let ctx = FilterContext {
        last_tool: Some("write".to_string()),
        ..Default::default()
    };

    let result = filter.filter(tools, &ctx);
    let names = tool_names(&result);
    assert_eq!(names, vec!["search", "read", "delete"]);
}

#[test]
fn turn_threshold_before() {
    let filter = StateMachineFilter::new().turn_threshold(3, ["advanced_tool"]);
    let tools = vec![make_tool("basic"), make_tool("advanced_tool")];

    let ctx = FilterContext {
        turn_count: 1,
        ..Default::default()
    };
    let result = filter.filter(tools, &ctx);
    let names = tool_names(&result);
    assert_eq!(names, vec!["basic"]); // advanced_tool hidden
}

#[test]
fn turn_threshold_after() {
    let filter = StateMachineFilter::new().turn_threshold(3, ["advanced_tool"]);
    let tools = vec![make_tool("basic"), make_tool("advanced_tool")];

    let ctx = FilterContext {
        turn_count: 3,
        ..Default::default()
    };
    let result = filter.filter(tools, &ctx);
    let names = tool_names(&result);
    assert_eq!(names, vec!["basic", "advanced_tool"]); // advanced_tool now visible
}

#[test]
fn composite_chains() {
    let filter = CompositeFilter::new(vec![
        Box::new(DenyListFilter::new(["delete"])),
        Box::new(AllowListFilter::new(["read"])),
    ]);
    let tools = vec![make_tool("read"), make_tool("write"), make_tool("delete")];
    let ctx = FilterContext::default();

    let result = filter.filter(tools, &ctx);
    let names = tool_names(&result);
    assert_eq!(names, vec!["read"]);
}

#[test]
fn allow_empty_result() {
    let filter = AllowListFilter::new(["nonexistent"]);
    let tools = vec![make_tool("read"), make_tool("write")];
    let ctx = FilterContext::default();

    let result = filter.filter(tools, &ctx);
    assert!(result.is_empty());
}

#[test]
fn deny_empty_list_passes_all() {
    let filter = DenyListFilter::new(Vec::<String>::new());
    let tools = vec![make_tool("read"), make_tool("write")];
    let ctx = FilterContext::default();

    let result = filter.filter(tools, &ctx);
    assert_eq!(tool_names(&result), vec!["read", "write"]);
}

#[test]
fn state_machine_no_last_tool_passes_all() {
    let filter = StateMachineFilter::new().after_tool("search", ["read"]);
    let tools = vec![make_tool("search"), make_tool("read"), make_tool("delete")];
    let ctx = FilterContext::default(); // last_tool is None

    let result = filter.filter(tools, &ctx);
    assert_eq!(tool_names(&result), vec!["search", "read", "delete"]);
}

#[test]
fn state_machine_combined_after_tool_and_threshold() {
    let filter = StateMachineFilter::new()
        .after_tool("search", ["read", "advanced"])
        .turn_threshold(5, ["advanced"]);

    let tools = vec![
        make_tool("search"),
        make_tool("read"),
        make_tool("advanced"),
    ];

    // last_tool = search (restricts to read + advanced), but turn_count < 5 (hides advanced)
    let ctx = FilterContext {
        turn_count: 2,
        last_tool: Some("search".to_string()),
        ..Default::default()
    };
    let result = filter.filter(tools.clone(), &ctx);
    assert_eq!(tool_names(&result), vec!["read"]);

    // last_tool = search, turn_count >= 5 => both read and advanced
    let ctx = FilterContext {
        turn_count: 5,
        last_tool: Some("search".to_string()),
        ..Default::default()
    };
    let result = filter.filter(tools, &ctx);
    assert_eq!(tool_names(&result), vec!["read", "advanced"]);
}
