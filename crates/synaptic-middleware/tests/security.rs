use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{json, Value};
use synaptic_core::{SynapticError, ToolCall};
use synaptic_middleware::{
    AgentMiddleware, ConfirmationPolicy, RiskLevel, RuleBasedAnalyzer, SecurityAnalyzer,
    SecurityConfirmationCallback, SecurityMiddleware, ThresholdConfirmationPolicy, ToolCallRequest,
    ToolCaller,
};

#[tokio::test]
async fn rule_based_risk() {
    let analyzer = RuleBasedAnalyzer::new()
        .with_tool_risk("delete_file", RiskLevel::High)
        .with_tool_risk("read_file", RiskLevel::None);

    let risk = analyzer.assess("delete_file", &json!({})).await.unwrap();
    assert_eq!(risk, RiskLevel::High);

    let risk = analyzer.assess("read_file", &json!({})).await.unwrap();
    assert_eq!(risk, RiskLevel::None);
}

#[tokio::test]
async fn default_unknown() {
    let analyzer = RuleBasedAnalyzer::new().with_default_risk(RiskLevel::Medium);
    let risk = analyzer.assess("unknown_tool", &json!({})).await.unwrap();
    assert_eq!(risk, RiskLevel::Medium);
}

#[tokio::test]
async fn arg_pattern_elevates() {
    let analyzer = RuleBasedAnalyzer::new()
        .with_tool_risk("execute", RiskLevel::Low)
        .with_arg_pattern("command", "rm -rf", RiskLevel::Critical);

    let risk = analyzer
        .assess("execute", &json!({"command": "rm -rf /"}))
        .await
        .unwrap();
    assert_eq!(risk, RiskLevel::Critical);

    // Without matching pattern, stays at base risk
    let risk = analyzer
        .assess("execute", &json!({"command": "ls"}))
        .await
        .unwrap();
    assert_eq!(risk, RiskLevel::Low);
}

#[tokio::test]
async fn threshold_confirms() {
    let policy = ThresholdConfirmationPolicy::new(RiskLevel::High);

    assert!(!policy.should_confirm("tool", RiskLevel::Low).await.unwrap());
    assert!(!policy
        .should_confirm("tool", RiskLevel::Medium)
        .await
        .unwrap());
    assert!(policy
        .should_confirm("tool", RiskLevel::High)
        .await
        .unwrap());
    assert!(policy
        .should_confirm("tool", RiskLevel::Critical)
        .await
        .unwrap());
}

/// A callback that always denies.
struct DenyCallback;

#[async_trait]
impl SecurityConfirmationCallback for DenyCallback {
    async fn confirm(&self, _: &str, _: &Value, _: RiskLevel) -> Result<bool, SynapticError> {
        Ok(false)
    }
}

/// A tool caller that returns success.
struct SuccessToolCaller;

#[async_trait]
impl ToolCaller for SuccessToolCaller {
    async fn call(&self, _request: ToolCallRequest) -> Result<Value, SynapticError> {
        Ok(json!({"status": "ok"}))
    }
}

fn make_request(name: &str) -> ToolCallRequest {
    ToolCallRequest {
        call: ToolCall {
            id: "call_1".to_string(),
            name: name.to_string(),
            arguments: json!({}),
        },
    }
}

#[tokio::test]
async fn middleware_blocks() {
    let mw = SecurityMiddleware::new(
        Arc::new(RuleBasedAnalyzer::new().with_tool_risk("danger", RiskLevel::High)),
        Arc::new(ThresholdConfirmationPolicy::new(RiskLevel::High)),
        Arc::new(DenyCallback),
    );

    let result = mw
        .wrap_tool_call(make_request("danger"), &SuccessToolCaller)
        .await;
    assert!(result.is_err());
}

#[tokio::test]
async fn middleware_passes() {
    let mw = SecurityMiddleware::new(
        Arc::new(RuleBasedAnalyzer::new().with_tool_risk("safe", RiskLevel::Low)),
        Arc::new(ThresholdConfirmationPolicy::new(RiskLevel::High)),
        Arc::new(DenyCallback), // Won't be called since risk is low
    );

    let result = mw
        .wrap_tool_call(make_request("safe"), &SuccessToolCaller)
        .await;
    assert!(result.is_ok());
}

#[tokio::test]
async fn bypass_exempted() {
    let mw = SecurityMiddleware::new(
        Arc::new(RuleBasedAnalyzer::new().with_tool_risk("danger", RiskLevel::Critical)),
        Arc::new(ThresholdConfirmationPolicy::new(RiskLevel::High)),
        Arc::new(DenyCallback),
    )
    .with_bypass(["danger"]);

    let result = mw
        .wrap_tool_call(make_request("danger"), &SuccessToolCaller)
        .await;
    assert!(result.is_ok()); // Bypassed!
}
