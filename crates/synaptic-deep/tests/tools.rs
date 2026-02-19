use serde_json::json;
use std::sync::Arc;
use synaptic_core::Tool;
use synaptic_deep::backend::{Backend, StateBackend};
use synaptic_deep::tools::create_filesystem_tools;

fn setup() -> (Arc<StateBackend>, Vec<Arc<dyn Tool>>) {
    let backend = Arc::new(StateBackend::new());
    let tools = create_filesystem_tools(backend.clone());
    (backend, tools)
}

fn find_tool<'a>(tools: &'a [Arc<dyn Tool>], name: &str) -> &'a Arc<dyn Tool> {
    tools.iter().find(|t| t.name() == name).unwrap()
}

#[tokio::test]
async fn tools_count() {
    let (_, tools) = setup();
    // StateBackend doesn't support execution, so 6 tools (no execute)
    assert_eq!(tools.len(), 6);
    let names: Vec<&str> = tools.iter().map(|t| t.name()).collect();
    assert!(names.contains(&"ls"));
    assert!(names.contains(&"read_file"));
    assert!(names.contains(&"write_file"));
    assert!(names.contains(&"edit_file"));
    assert!(names.contains(&"glob"));
    assert!(names.contains(&"grep"));
}

#[tokio::test]
async fn ls_tool() {
    let (backend, tools) = setup();
    backend.write_file("src/main.rs", "fn main() {}").await.unwrap();

    let ls = find_tool(&tools, "ls");
    let result = ls.call(json!({"path": "."})).await.unwrap();
    let entries: Vec<serde_json::Value> = serde_json::from_value(result).unwrap();
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0]["name"], "src");
    assert_eq!(entries[0]["is_dir"], true);
}

#[tokio::test]
async fn write_and_read_tool() {
    let (_, tools) = setup();
    let write = find_tool(&tools, "write_file");
    let read = find_tool(&tools, "read_file");

    let result = write.call(json!({"path": "test.txt", "content": "hello world"})).await.unwrap();
    assert!(result.as_str().unwrap().contains("wrote"));

    let content = read.call(json!({"path": "test.txt"})).await.unwrap();
    assert_eq!(content.as_str().unwrap(), "hello world");
}

#[tokio::test]
async fn read_with_pagination() {
    let (backend, tools) = setup();
    backend.write_file("lines.txt", "a\nb\nc\nd\ne").await.unwrap();

    let read = find_tool(&tools, "read_file");
    let content = read.call(json!({"path": "lines.txt", "offset": 1, "limit": 2})).await.unwrap();
    assert_eq!(content.as_str().unwrap(), "b\nc");
}

#[tokio::test]
async fn edit_tool() {
    let (backend, tools) = setup();
    backend.write_file("f.txt", "hello world").await.unwrap();

    let edit = find_tool(&tools, "edit_file");
    edit.call(json!({
        "path": "f.txt",
        "old_string": "hello",
        "new_string": "goodbye"
    })).await.unwrap();

    let read = find_tool(&tools, "read_file");
    let content = read.call(json!({"path": "f.txt"})).await.unwrap();
    assert_eq!(content.as_str().unwrap(), "goodbye world");
}

#[tokio::test]
async fn glob_tool() {
    let (backend, tools) = setup();
    backend.write_file("src/a.rs", "").await.unwrap();
    backend.write_file("src/b.txt", "").await.unwrap();

    let glob = find_tool(&tools, "glob");
    let result = glob.call(json!({"pattern": "*.rs", "path": "src"})).await.unwrap();
    assert_eq!(result.as_str().unwrap(), "src/a.rs");
}

#[tokio::test]
async fn grep_tool() {
    let (backend, tools) = setup();
    backend.write_file("a.txt", "hello world\ngoodbye").await.unwrap();
    backend.write_file("b.txt", "no match").await.unwrap();

    let grep = find_tool(&tools, "grep");
    let result = grep.call(json!({"pattern": "hello"})).await.unwrap();
    assert_eq!(result.as_str().unwrap(), "a.txt");
}

#[tokio::test]
async fn grep_tool_content_mode() {
    let (backend, tools) = setup();
    backend.write_file("f.txt", "aaa\nbbb\nccc").await.unwrap();

    let grep = find_tool(&tools, "grep");
    let result = grep.call(json!({"pattern": "bbb", "output_mode": "content"})).await.unwrap();
    assert_eq!(result.as_str().unwrap(), "f.txt:2:bbb");
}

#[tokio::test]
async fn read_missing_path_param() {
    let (_, tools) = setup();
    let read = find_tool(&tools, "read_file");
    let err = read.call(json!({})).await;
    assert!(err.is_err());
}

#[tokio::test]
async fn write_missing_content_param() {
    let (_, tools) = setup();
    let write = find_tool(&tools, "write_file");
    let err = write.call(json!({"path": "f.txt"})).await;
    assert!(err.is_err());
}

#[tokio::test]
async fn all_tools_have_parameters() {
    let (_, tools) = setup();
    for tool in &tools {
        assert!(tool.parameters().is_some(), "tool {} missing parameters", tool.name());
    }
}
