//! Workflow CLI commands — list, run, status, approve, reject.
//!
//! Discovers workflow definitions from `.claude/workflows/` (TOML files) and
//! executes them via the synaptic-graph `WorkflowRunner` with checkpoint-backed
//! pause/resume.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use colored::Colorize;
use serde::Deserialize;
use serde_json::Value;
use synaptic::core::{ChatModel, ChatRequest, ChatResponse, Message, SynapticError};
use synaptic::graph::workflow::{
    Workflow, WorkflowContext, WorkflowError, WorkflowHandler, WorkflowResult, WorkflowStatus,
    WorkflowStep,
};
use synaptic::graph::workflow_runner::WorkflowRunner;
use synaptic::graph::{CheckpointConfig, Checkpointer, StoreCheckpointer};
use synaptic::store::FileStore;

use crate::config::SynapseConfig;

// ---------------------------------------------------------------------------
// Workflow definition (TOML format)
// ---------------------------------------------------------------------------

/// TOML-serializable workflow definition loaded from `.claude/workflows/`.
#[derive(Debug, Clone, Deserialize)]
struct WorkflowDef {
    name: String,
    description: Option<String>,
    steps: Vec<WorkflowStepDef>,
}

/// A single step definition in a workflow TOML.
#[derive(Debug, Clone, Deserialize)]
#[allow(dead_code)]
struct WorkflowStepDef {
    name: String,
    /// What this step does (displayed to user).
    description: Option<String>,
    /// Whether this step pauses for human approval before executing.
    #[serde(default)]
    requires_approval: bool,
    /// Shell command to run for this step (simple handler).
    command: Option<String>,
    /// Static message to emit (for steps that are just checkpoints).
    message: Option<String>,

    // --- LlmCall step fields ---
    /// Prompt to send to the LLM (enables LlmCall step kind).
    /// Supports `{key}` placeholder substitution from workflow state.
    prompt: Option<String>,
    /// Model to use for LlmCall (optional; defaults to config's default model).
    model: Option<String>,

    // --- Condition step fields ---
    /// State field to read for comparison (enables Condition step kind).
    field: Option<String>,
    /// Comparison operator: "eq", "ne", "gt", "lt", "contains".
    operator: Option<String>,
    /// Value to compare against.
    value: Option<Value>,
    /// Step name to branch to when condition is true (optional).
    branch_true: Option<String>,
    /// Step name to branch to when condition is false (optional).
    branch_false: Option<String>,

    // --- Delay step fields ---
    /// Duration in seconds to pause execution (enables Delay step kind).
    delay_secs: Option<u64>,
}

// ---------------------------------------------------------------------------
// StepKind — discriminates which handler to build for a step
// ---------------------------------------------------------------------------

/// The resolved kind of a workflow step.
#[derive(Debug, Clone)]
pub enum StepKind {
    /// Run a shell command or emit a static message (default).
    Shell,
    /// Call an LLM with a prompt and store the result in workflow state.
    LlmCall {
        model: Option<String>,
        prompt: String,
    },
    /// Evaluate a simple comparison against workflow state for branching.
    Condition {
        field: String,
        operator: String,
        value: Value,
        branch_true: Option<String>,
        branch_false: Option<String>,
    },
    /// Pause workflow execution for a fixed duration.
    Delay { duration_secs: u64 },
}

// ---------------------------------------------------------------------------
// Registry — discovers and loads workflow definitions
// ---------------------------------------------------------------------------

struct WorkflowRegistry {
    workflows: Vec<WorkflowDef>,
}

impl WorkflowRegistry {
    /// Discover workflows from standard directories.
    fn discover() -> Self {
        let mut workflows = Vec::new();
        let cwd = std::env::current_dir().unwrap_or_default();

        // Project-level: .claude/workflows/
        let project_dir = cwd.join(".claude/workflows");
        Self::scan_dir(&project_dir, &mut workflows);

        // Global: ~/.claude/workflows/
        if let Some(home) = dirs::home_dir() {
            let global_dir = home.join(".claude/workflows");
            Self::scan_dir(&global_dir, &mut workflows);
        }

        Self { workflows }
    }

    fn scan_dir(dir: &Path, out: &mut Vec<WorkflowDef>) {
        let entries = match std::fs::read_dir(dir) {
            Ok(e) => e,
            Err(_) => return,
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|e| e.to_str()) == Some("toml") {
                if let Ok(content) = std::fs::read_to_string(&path) {
                    match toml::from_str::<WorkflowDef>(&content) {
                        Ok(def) => out.push(def),
                        Err(e) => {
                            eprintln!(
                                "{} Failed to parse {}: {}",
                                "warning:".yellow().bold(),
                                path.display(),
                                e
                            );
                        }
                    }
                }
            }
        }
    }

    fn get(&self, name: &str) -> Option<&WorkflowDef> {
        self.workflows.iter().find(|w| w.name == name)
    }
}

// ---------------------------------------------------------------------------
// Step handler — executes shell commands or emits messages
// ---------------------------------------------------------------------------

struct ShellStepHandler {
    command: Option<String>,
    message: Option<String>,
    work_dir: PathBuf,
}

#[async_trait::async_trait]
impl WorkflowHandler for ShellStepHandler {
    async fn execute(&self, ctx: &mut WorkflowContext) -> Result<WorkflowResult, WorkflowError> {
        if let Some(ref cmd) = self.command {
            let output = tokio::process::Command::new("sh")
                .arg("-c")
                .arg(cmd)
                .current_dir(&self.work_dir)
                .output()
                .await
                .map_err(|e| WorkflowError::Other(format!("command failed: {}", e)))?;

            let stdout = String::from_utf8_lossy(&output.stdout).to_string();
            let stderr = String::from_utf8_lossy(&output.stderr).to_string();

            if !stdout.is_empty() {
                eprintln!("{}", stdout.trim());
            }
            if !stderr.is_empty() {
                eprintln!("{}", stderr.trim());
            }

            if !output.status.success() {
                return Err(WorkflowError::Other(format!(
                    "command exited with {}",
                    output.status
                )));
            }

            // Merge stdout into state
            let mut state = ctx.state.clone();
            if let Value::Object(ref mut map) = state {
                map.insert("last_output".to_string(), Value::String(stdout));
            }
            Ok(WorkflowResult::Continue(state))
        } else if let Some(ref msg) = self.message {
            eprintln!("{}", msg);
            Ok(WorkflowResult::Continue(ctx.state.clone()))
        } else {
            // No-op step (just a checkpoint)
            Ok(WorkflowResult::Continue(ctx.state.clone()))
        }
    }
}

// ---------------------------------------------------------------------------
// LlmCallHandler — calls a language model within a workflow step
// ---------------------------------------------------------------------------

struct LlmCallHandler {
    model: Arc<dyn ChatModel>,
    model_name: Option<String>,
    prompt: String,
}

#[async_trait::async_trait]
impl WorkflowHandler for LlmCallHandler {
    async fn execute(&self, ctx: &mut WorkflowContext) -> Result<WorkflowResult, WorkflowError> {
        // Interpolate {key} placeholders from workflow state into prompt
        let prompt = interpolate_prompt(&self.prompt, &ctx.state);

        tracing::info!(
            model = ?self.model_name,
            prompt_len = prompt.len(),
            "workflow LlmCall step executing"
        );

        let request = ChatRequest::new(vec![Message::human(prompt)]);
        let response = self
            .model
            .chat(request)
            .await
            .map_err(|e| WorkflowError::Other(format!("LLM call failed: {}", e)))?;

        let result_text = response.message.content().to_string();

        eprintln!("{}", result_text);

        // Store result in state under "llm_result"
        let mut state = ctx.state.clone();
        if let Value::Object(ref mut map) = state {
            map.insert("llm_result".to_string(), Value::String(result_text));
        }
        Ok(WorkflowResult::Continue(state))
    }
}

/// Interpolate `{key}` placeholders in `template` from JSON state object.
fn interpolate_prompt(template: &str, state: &Value) -> String {
    let mut result = template.to_string();
    if let Value::Object(map) = state {
        for (key, val) in map {
            let placeholder = format!("{{{}}}", key);
            let replacement = match val {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            result = result.replace(&placeholder, &replacement);
        }
    }
    result
}

// ---------------------------------------------------------------------------
// ConditionHandler — evaluates a comparison and optionally branches
// ---------------------------------------------------------------------------

struct ConditionHandler {
    field: String,
    operator: String,
    value: Value,
    branch_true: Option<String>,
    branch_false: Option<String>,
}

#[async_trait::async_trait]
impl WorkflowHandler for ConditionHandler {
    async fn execute(&self, ctx: &mut WorkflowContext) -> Result<WorkflowResult, WorkflowError> {
        let field_val = match &ctx.state {
            Value::Object(map) => map.get(&self.field).cloned().unwrap_or(Value::Null),
            _ => Value::Null,
        };

        let result = evaluate_condition(&field_val, &self.operator, &self.value);

        tracing::info!(
            field = %self.field,
            operator = %self.operator,
            result = result,
            "workflow Condition step evaluated"
        );

        // Store condition result in state
        let mut state = ctx.state.clone();
        if let Value::Object(ref mut map) = state {
            map.insert("condition_result".to_string(), Value::Bool(result));
        }

        match result {
            true => match &self.branch_true {
                Some(target) => Ok(WorkflowResult::Branch(target.clone())),
                None => Ok(WorkflowResult::Continue(state)),
            },
            false => match &self.branch_false {
                Some(target) => Ok(WorkflowResult::Branch(target.clone())),
                None => Ok(WorkflowResult::Continue(state)),
            },
        }
    }
}

/// Evaluate a condition: `field_val <operator> cmp_val`.
///
/// Supported operators: `eq`, `ne`, `gt`, `lt`, `contains`.
fn evaluate_condition(field_val: &Value, operator: &str, cmp_val: &Value) -> bool {
    match operator {
        "eq" => field_val == cmp_val,
        "ne" => field_val != cmp_val,
        "gt" => match (field_val, cmp_val) {
            (Value::Number(a), Value::Number(b)) => {
                a.as_f64().unwrap_or(0.0) > b.as_f64().unwrap_or(0.0)
            }
            (Value::String(a), Value::String(b)) => a > b,
            _ => false,
        },
        "lt" => match (field_val, cmp_val) {
            (Value::Number(a), Value::Number(b)) => {
                a.as_f64().unwrap_or(0.0) < b.as_f64().unwrap_or(0.0)
            }
            (Value::String(a), Value::String(b)) => a < b,
            _ => false,
        },
        "contains" => match (field_val, cmp_val) {
            (Value::String(haystack), Value::String(needle)) => haystack.contains(needle.as_str()),
            (Value::Array(arr), needle) => arr.contains(needle),
            _ => false,
        },
        _ => {
            tracing::warn!(operator = %operator, "unknown condition operator — treating as false");
            false
        }
    }
}

// ---------------------------------------------------------------------------
// DelayHandler — pauses workflow execution for a fixed duration
// ---------------------------------------------------------------------------

struct DelayHandler {
    duration_secs: u64,
}

#[async_trait::async_trait]
impl WorkflowHandler for DelayHandler {
    async fn execute(&self, ctx: &mut WorkflowContext) -> Result<WorkflowResult, WorkflowError> {
        tracing::info!(
            duration_secs = self.duration_secs,
            "workflow Delay step sleeping"
        );
        tokio::time::sleep(Duration::from_secs(self.duration_secs)).await;
        Ok(WorkflowResult::Continue(ctx.state.clone()))
    }
}

// ---------------------------------------------------------------------------
// NoOpModel — fallback ChatModel when LLM config is unavailable at build time
// ---------------------------------------------------------------------------

struct NoOpModel;

#[async_trait::async_trait]
impl ChatModel for NoOpModel {
    async fn chat(&self, _request: ChatRequest) -> Result<ChatResponse, SynapticError> {
        Ok(ChatResponse {
            message: Message::ai("(LLM unavailable — no model configured for this workflow step)"),
            usage: None,
        })
    }
}

// ---------------------------------------------------------------------------
// resolve_step_kind — determines which StepKind applies to a step definition
// ---------------------------------------------------------------------------

fn resolve_step_kind(s: &WorkflowStepDef) -> StepKind {
    // Delay: delay_secs is set
    if let Some(duration_secs) = s.delay_secs {
        return StepKind::Delay { duration_secs };
    }

    // Condition: field + operator + value are all set
    if let (Some(field), Some(operator), Some(value)) =
        (s.field.clone(), s.operator.clone(), s.value.clone())
    {
        return StepKind::Condition {
            field,
            operator,
            value,
            branch_true: s.branch_true.clone(),
            branch_false: s.branch_false.clone(),
        };
    }

    // LlmCall: prompt is set
    if let Some(prompt) = s.prompt.clone() {
        return StepKind::LlmCall {
            model: s.model.clone(),
            prompt,
        };
    }

    // Default: shell / message / no-op
    StepKind::Shell
}

// ---------------------------------------------------------------------------
// Build a Workflow from a WorkflowDef
// ---------------------------------------------------------------------------

fn build_workflow(def: &WorkflowDef, config: &SynapseConfig) -> Workflow {
    let work_dir = std::env::current_dir().unwrap_or_default();
    let steps = def
        .steps
        .iter()
        .map(|s| {
            let kind = resolve_step_kind(s);
            let handler: Box<dyn WorkflowHandler> = match kind {
                StepKind::LlmCall {
                    ref model,
                    ref prompt,
                } => {
                    let model_arc = crate::agent::build_model(config, model.as_deref())
                        .unwrap_or_else(|e| {
                            tracing::warn!(
                                error = %e,
                                "failed to build LLM for workflow step — falling back to no-op"
                            );
                            Arc::new(NoOpModel)
                        });
                    Box::new(LlmCallHandler {
                        model: model_arc,
                        model_name: model.clone(),
                        prompt: prompt.clone(),
                    })
                }
                StepKind::Condition {
                    ref field,
                    ref operator,
                    ref value,
                    ref branch_true,
                    ref branch_false,
                } => Box::new(ConditionHandler {
                    field: field.clone(),
                    operator: operator.clone(),
                    value: value.clone(),
                    branch_true: branch_true.clone(),
                    branch_false: branch_false.clone(),
                }),
                StepKind::Delay { duration_secs } => Box::new(DelayHandler { duration_secs }),
                StepKind::Shell => Box::new(ShellStepHandler {
                    command: s.command.clone(),
                    message: s.message.clone(),
                    work_dir: work_dir.clone(),
                }),
            };
            WorkflowStep {
                name: s.name.clone(),
                handler,
                requires_approval: s.requires_approval,
                timeout: None,
            }
        })
        .collect();

    Workflow {
        name: def.name.clone(),
        description: def.description.clone().unwrap_or_default(),
        steps,
    }
}

// ---------------------------------------------------------------------------
// Create a checkpointer for workflow state persistence
// ---------------------------------------------------------------------------

fn workflow_checkpointer() -> Arc<dyn Checkpointer> {
    let base_dir = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".synapse/workflows");
    let _ = std::fs::create_dir_all(&base_dir);
    let store = Arc::new(FileStore::new(base_dir));
    Arc::new(StoreCheckpointer::new(store))
}

// ---------------------------------------------------------------------------
// CLI entry point
// ---------------------------------------------------------------------------

/// Run a workflow CLI command.
pub async fn run_workflow_command(
    config: &SynapseConfig,
    action: &str,
    name: Option<&str>,
    input: Option<&str>,
    data: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        "list" | "ls" => {
            let registry = WorkflowRegistry::discover();
            if registry.workflows.is_empty() {
                println!("{}", "No workflows found.".dimmed());
                println!("  Project:  .claude/workflows/*.toml");
                if let Some(home) = dirs::home_dir() {
                    println!("  Global:   {}/.claude/workflows/*.toml", home.display());
                }
                println!("\nCreate a workflow TOML file to get started. Example:");
                println!("{}", EXAMPLE_WORKFLOW.dimmed());
            } else {
                println!("{:<25} {:<6} DESCRIPTION", "NAME", "STEPS");
                println!("{}", "-".repeat(65));
                for wf in &registry.workflows {
                    let desc = wf.description.as_deref().unwrap_or("");
                    println!("{:<25} {:<6} {}", wf.name, wf.steps.len(), desc);
                }
                println!("\n{} workflow(s) found", registry.workflows.len());
            }
        }
        "run" => {
            let name = name.ok_or("usage: synapse workflow run <name> [--input json]")?;
            let registry = WorkflowRegistry::discover();
            let def = registry
                .get(name)
                .ok_or_else(|| format!("workflow '{}' not found", name))?;

            let input_value: Value = if let Some(json_str) = input {
                serde_json::from_str(json_str)
                    .map_err(|e| format!("invalid --input JSON: {}", e))?
            } else {
                Value::Object(serde_json::Map::new())
            };

            let workflow = build_workflow(def, config);
            let runner = WorkflowRunner::new(workflow_checkpointer());

            eprintln!(
                "{} Starting workflow '{}' ({} steps)...",
                "workflow:".cyan().bold(),
                name,
                def.steps.len()
            );

            let execution = runner
                .start(&workflow, input_value)
                .await
                .map_err(|e| format!("workflow execution failed: {}", e))?;

            print_execution_result(&execution.status, &execution.resume_token);
        }
        "status" => {
            let token = name.ok_or("usage: synapse workflow status <resume_token>")?;
            let runner = WorkflowRunner::new(workflow_checkpointer());

            match runner.status(token).await {
                Ok(status) => print_execution_result(&status, token),
                Err(WorkflowError::InvalidResumeToken) => {
                    println!(
                        "{} No workflow found for token '{}'",
                        "error:".red().bold(),
                        token
                    );
                }
                Err(e) => return Err(format!("status query failed: {}", e).into()),
            }
        }
        "approve" => {
            let token =
                name.ok_or("usage: synapse workflow approve <resume_token> [--data json]")?;

            let approval_data: Option<Value> = if let Some(json_str) = data {
                Some(
                    serde_json::from_str(json_str)
                        .map_err(|e| format!("invalid --data JSON: {}", e))?,
                )
            } else {
                None
            };

            // We need the workflow definition to resume — look it up from checkpoint
            let runner = WorkflowRunner::new(workflow_checkpointer());

            // Get the current status to find the workflow name
            let status = runner
                .status(token)
                .await
                .map_err(|e| format!("failed to query workflow: {}", e))?;

            match status {
                WorkflowStatus::WaitingApproval { ref step, .. } => {
                    eprintln!(
                        "{} Approving step '{}' (token: {})...",
                        "workflow:".green().bold(),
                        step,
                        token
                    );

                    // We need to find the workflow definition to resume
                    // The checkpoint stores the workflow name but we need the full definition
                    let registry = WorkflowRegistry::discover();
                    let wf_name = find_workflow_name_from_token(&runner, token).await?;
                    let def = registry.get(&wf_name).ok_or_else(|| {
                        format!("workflow '{}' not found (needed for resume)", wf_name)
                    })?;

                    let workflow = build_workflow(def, config);
                    let execution = runner
                        .resume(&workflow, token, approval_data)
                        .await
                        .map_err(|e| format!("resume failed: {}", e))?;

                    print_execution_result(&execution.status, &execution.resume_token);
                }
                other => {
                    println!(
                        "{} Workflow is not waiting for approval (current: {:?})",
                        "error:".red().bold(),
                        status_label(&other)
                    );
                }
            }
        }
        "reject" => {
            let token = name.ok_or("usage: synapse workflow reject <resume_token>")?;
            let runner = WorkflowRunner::new(workflow_checkpointer());

            let status = runner
                .status(token)
                .await
                .map_err(|e| format!("failed to query workflow: {}", e))?;

            match status {
                WorkflowStatus::WaitingApproval { ref step, .. } => {
                    eprintln!(
                        "{} Rejected step '{}' (token: {})",
                        "workflow:".red().bold(),
                        step,
                        token
                    );
                    // For rejection, we don't resume — we just inform the user.
                    // A future enhancement could store a "rejected" status in the checkpoint.
                    println!("Workflow paused. Use 'approve' to continue or discard the token.");
                }
                other => {
                    println!(
                        "{} Workflow is not waiting for approval (current: {})",
                        "error:".red().bold(),
                        status_label(&other)
                    );
                }
            }
        }
        _ => {
            return Err(format!(
                "unknown workflow action: '{}'. Use: list, run, status, approve, reject",
                action
            )
            .into());
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn print_execution_result(status: &WorkflowStatus, token: &str) {
    match status {
        WorkflowStatus::Running { step } => {
            println!("{} Running step '{}'", "workflow:".cyan().bold(), step);
        }
        WorkflowStatus::WaitingApproval {
            step,
            prompt,
            resume_token,
        } => {
            println!(
                "{} Paused at step '{}': {}",
                "workflow:".yellow().bold(),
                step,
                prompt
            );
            println!("  Resume token: {}", resume_token.cyan());
            println!("  Approve: synapse workflow approve {}", resume_token);
            println!("  Reject:  synapse workflow reject {}", resume_token);
        }
        WorkflowStatus::Completed { output } => {
            println!("{} Workflow completed", "workflow:".green().bold());
            if !output.is_null() {
                println!(
                    "  Output: {}",
                    serde_json::to_string_pretty(output).unwrap_or_default()
                );
            }
        }
        WorkflowStatus::Failed { error } => {
            println!("{} Workflow failed: {}", "workflow:".red().bold(), error);
            println!("  Token: {}", token);
        }
    }
}

fn status_label(status: &WorkflowStatus) -> &str {
    match status {
        WorkflowStatus::Running { .. } => "running",
        WorkflowStatus::WaitingApproval { .. } => "waiting_approval",
        WorkflowStatus::Completed { .. } => "completed",
        WorkflowStatus::Failed { .. } => "failed",
    }
}

/// Extract workflow name from checkpoint state for resume.
async fn find_workflow_name_from_token(
    _runner: &WorkflowRunner,
    token: &str,
) -> Result<String, Box<dyn std::error::Error>> {
    // The WorkflowRunner stores WorkflowCheckpoint with workflow_name in the checkpoint.
    // We query the checkpoint via status. The checkpoint state is a WorkflowCheckpoint.
    // Since we can't directly access the internal checkpoint, we use a heuristic:
    // re-read the checkpoint via the checkpointer.
    let checkpointer = workflow_checkpointer();
    let config = CheckpointConfig {
        thread_id: format!("workflow:{}", token),
        checkpoint_id: None,
    };
    let cp = checkpointer
        .get(&config)
        .await
        .map_err(|e| format!("checkpoint read failed: {}", e))?
        .ok_or("no checkpoint found for this token")?;

    // Parse the workflow_name from the checkpoint state
    let state = cp.state;
    let wf_name = state
        .get("workflow_name")
        .and_then(|v| v.as_str())
        .ok_or("checkpoint missing workflow_name")?;

    Ok(wf_name.to_string())
}

const EXAMPLE_WORKFLOW: &str = r#"
  # .claude/workflows/deploy-review.toml
  name = "deploy-review"
  description = "Review and deploy with approval gate"

  [[steps]]
  name = "lint"
  command = "cargo clippy --all-targets"

  [[steps]]
  name = "test"
  command = "cargo test"

  [[steps]]
  name = "review"
  requires_approval = true
  message = "All checks passed. Ready to deploy?"

  [[steps]]
  name = "deploy"
  command = "echo 'Deploying...'"

  # Example: LlmCall step
  # [[steps]]
  # name = "summarize"
  # prompt = "Summarize the test results: {last_output}"
  # model = "gpt-4o-mini"   # optional; omit to use default model

  # Example: Condition step
  # [[steps]]
  # name = "check-env"
  # field = "env"
  # operator = "eq"
  # value = "production"
  # branch_true = "deploy-prod"
  # branch_false = "deploy-staging"

  # Example: Delay step
  # [[steps]]
  # name = "cooldown"
  # delay_secs = 30
"#;
