use serde_json::json;
use std::sync::Arc;
use synaptic_core::{SynapticError, Tool};
use synaptic_tools::ToolRegistry;

struct CounterTool {
    name: &'static str,
}

#[async_trait::async_trait]
impl Tool for CounterTool {
    fn name(&self) -> &'static str {
        self.name
    }
    fn description(&self) -> &'static str {
        "A counter tool"
    }
    async fn call(&self, _args: serde_json::Value) -> Result<serde_json::Value, SynapticError> {
        Ok(json!(self.name))
    }
}

#[test]
fn registry_register_and_lookup() {
    let registry = ToolRegistry::new();
    let tool = Arc::new(CounterTool { name: "alpha" });
    registry.register(tool).unwrap();

    let found = registry.get("alpha");
    assert!(found.is_some());
    assert_eq!(found.unwrap().name(), "alpha");
}

#[test]
fn registry_lookup_missing() {
    let registry = ToolRegistry::new();
    assert!(registry.get("nonexistent").is_none());
}

#[test]
fn registry_duplicate_overwrites() {
    let registry = ToolRegistry::new();
    let tool1 = Arc::new(CounterTool { name: "tool" });
    let tool2 = Arc::new(CounterTool { name: "tool" });
    registry.register(tool1).unwrap();
    registry.register(tool2).unwrap();

    // Should have one entry (overwritten)
    let found = registry.get("tool");
    assert!(found.is_some());
}

#[tokio::test]
async fn registry_concurrent_register_and_get() {
    let registry = ToolRegistry::new();

    // Register several tools first
    for i in 0..10 {
        let name: &'static str = Box::leak(format!("tool_{i}").into_boxed_str());
        registry.register(Arc::new(CounterTool { name })).unwrap();
    }

    // Concurrent reads
    let mut handles = Vec::new();
    for i in 0..10 {
        let r = registry.clone();
        let name = format!("tool_{i}");
        handles.push(tokio::spawn(async move {
            let found = r.get(&name);
            assert!(found.is_some());
        }));
    }

    for h in handles {
        h.await.unwrap();
    }
}

#[test]
fn registry_clone_shares_state() {
    let registry = ToolRegistry::new();
    let tool = Arc::new(CounterTool { name: "shared" });
    registry.register(tool).unwrap();

    let cloned = registry.clone();
    assert!(cloned.get("shared").is_some());
}

#[test]
fn registry_empty_by_default() {
    let registry = ToolRegistry::new();
    assert!(registry.get("anything").is_none());
}
