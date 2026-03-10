use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use synaptic::condenser::{
    ChunkedSummarizingCondenser, CondenserMiddleware, Condenser, LlmSummarizingCondenser,
    TokenBudgetCondenser,
};
use synaptic::core::{ChatModel, SynapticError, TokenCounter, Tool};
use synaptic::deep::{create_deep_agent, DeepAgentOptions};
use synaptic::graph::{Checkpointer, CompiledGraph, MessageState};
use synaptic::middleware::{
    CircuitBreakerConfig, CircuitBreakerMiddleware, RiskLevel, RuleBasedAnalyzer,
    SecurityConfirmationCallback, SecurityMiddleware, SsrfGuardConfig, SsrfGuardMiddleware,
    ThresholdConfirmationPolicy,
};
use synaptic::callbacks::CostTrackingCallback;
use synaptic::secrets::{SecretMaskingMiddleware, SecretRegistry};
use synaptic_deep::backend::FilesystemBackend;
use synaptic_deep::CommandExecutor;

use crate::config::SynapseConfig;

use super::callbacks::AutoApproveCallback;
use super::context::load_project_context;
use super::discovery::discover_agents;
use super::middleware::{LoopDetectionMiddleware, build_fallback_middleware};
use super::thinking::{ThinkingMiddleware, VerboseMiddleware};
use super::tracing_mw::AgentTracingMiddleware;

/// Lightweight middleware that records token usage on every LLM response.
struct CostTrackingMw(Arc<CostTrackingCallback>);

#[async_trait]
impl synaptic::middleware::AgentMiddleware for CostTrackingMw {
    async fn after_model(
        &self,
        _request: &synaptic::middleware::ModelRequest,
        response: &mut synaptic::middleware::ModelResponse,
    ) -> Result<(), SynapticError> {
        match response.usage {
            Some(ref usage) => {
                tracing::debug!(
                    input = usage.input_tokens,
                    output = usage.output_tokens,
                    total = usage.total_tokens,
                    "Token usage"
                );
                self.0.record_usage(usage).await;
            }
            None => {
                tracing::debug!("Provider returned no usage data for this response");
            }
        }
        Ok(())
    }
}

/// Shell command executor for resolving !`command` placeholders in SKILL.md.
struct ShellCommandExecutor {
    work_dir: std::path::PathBuf,
}

impl ShellCommandExecutor {
    fn new(work_dir: &Path) -> Self {
        Self {
            work_dir: work_dir.to_path_buf(),
        }
    }
}

#[async_trait]
impl CommandExecutor for ShellCommandExecutor {
    async fn execute(&self, command: &str) -> Result<String, SynapticError> {
        let output = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .current_dir(&self.work_dir)
            .output()
            .await
            .map_err(|e| SynapticError::Tool(format!("command execution failed: {}", e)))?;
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

/// Session-level overrides for thinking and verbose modes.
#[derive(Debug, Clone, Default)]
pub struct SessionOverrides {
    /// Thinking level: "off", "low", "medium", "high", "adaptive", or a token budget number.
    pub thinking: Option<String>,
    /// Verbose level: "off", "on", "full", "inherit".
    pub verbose: Option<String>,
}

/// Build a Deep Agent with real filesystem backend, MCP tools, and middleware.
#[allow(dead_code)]
pub async fn build_deep_agent(
    model: Arc<dyn ChatModel>,
    config: &SynapseConfig,
    cwd: &Path,
    checkpointer: Arc<dyn Checkpointer>,
    mcp_tools: Vec<Arc<dyn Tool>>,
    system_prompt_override: Option<&str>,
) -> Result<CompiledGraph<MessageState>, SynapticError> {
    build_deep_agent_with_callback(
        model,
        config,
        cwd,
        checkpointer,
        mcp_tools,
        system_prompt_override,
        None,
        None,
        None,
        None,
        None,
    )
    .await
}

/// Build a Deep Agent with an optional custom security callback and optional LTM.
pub async fn build_deep_agent_with_callback(
    model: Arc<dyn ChatModel>,
    config: &SynapseConfig,
    cwd: &Path,
    checkpointer: Arc<dyn Checkpointer>,
    mcp_tools: Vec<Arc<dyn Tool>>,
    system_prompt_override: Option<&str>,
    security_callback: Option<Arc<dyn SecurityConfirmationCallback>>,
    ltm: Option<Arc<crate::memory::LongTermMemory>>,
    session_mgr: Option<Arc<synaptic::session::SessionManager>>,
    session_overrides: Option<SessionOverrides>,
    cost_tracker: Option<Arc<CostTrackingCallback>>,
) -> Result<CompiledGraph<MessageState>, SynapticError> {
    // Use docker backend if configured, otherwise filesystem
    #[cfg(feature = "docker")]
    let backend: Arc<dyn synaptic_deep::backend::Backend> = {
        if let Some(ref docker_cfg) = config.docker {
            if docker_cfg.enabled {
                let image = docker_cfg
                    .image
                    .as_deref()
                    .unwrap_or("ubuntu:22.04");
                let work_dir = cwd.to_string_lossy();
                tracing::info!(backend = "docker", image = %image, "Creating Docker container");
                match crate::docker::manager::DockerManager::create_workspace_with_mount(
                    image,
                    &work_dir,
                    &work_dir,
                )
                .await
                {
                    Ok(workspace) => {
                        tracing::info!(backend = "docker", "Docker sandbox ready");
                        Arc::new(workspace)
                    }
                    Err(e) => {
                        tracing::warn!(backend = "docker", error = %e, "Docker setup failed, falling back to filesystem");
                        Arc::new(FilesystemBackend::new(cwd))
                    }
                }
            } else {
                Arc::new(FilesystemBackend::new(cwd))
            }
        } else {
            Arc::new(FilesystemBackend::new(cwd))
        }
    };
    #[cfg(not(feature = "docker"))]
    let backend: Arc<dyn synaptic_deep::backend::Backend> =
        Arc::new(FilesystemBackend::new(cwd));

    let mut options = DeepAgentOptions::new(backend.clone());

    // System prompt + project context
    let mut system_prompt = system_prompt_override
        .map(|s| s.to_string())
        .or_else(|| config.base.agent.system_prompt.clone())
        .unwrap_or_else(|| {
            "You are Synapse, a helpful AI assistant powered by the Synaptic framework.".to_string()
        });

    let project_context = load_project_context(cwd, &config.context);
    if !project_context.is_empty() {
        system_prompt.push_str("\n\n# Project Context\n\n");
        system_prompt.push_str(&project_context);
    }

    options.system_prompt = Some(system_prompt);
    options.enable_filesystem = config.base.agent.tools.filesystem;
    options.memory_file = Some(config.base.paths.memory_file.clone());

    // Build skills_dirs: personal > project > legacy > custom config
    {
        let mut skills_dirs = Vec::new();
        if let Some(home) = dirs::home_dir() {
            let personal = home.join(".claude/skills");
            skills_dirs.push(personal.to_string_lossy().to_string());
        }
        skills_dirs.push(cwd.join(".claude/skills").to_string_lossy().to_string());
        skills_dirs.push(cwd.join(".claude/commands").to_string_lossy().to_string());
        if config.base.paths.skills_dir != ".claude/skills" {
            skills_dirs.push(
                cwd.join(&config.base.paths.skills_dir)
                    .to_string_lossy()
                    .to_string(),
            );
        }
        options.skills_dirs = skills_dirs;
    }

    // ShellCommandExecutor for skill !`command` placeholders
    options.command_executor = Some(Arc::new(ShellCommandExecutor::new(cwd)));
    options.checkpointer = Some(checkpointer);

    // Wire up subagent config
    options.enable_subagents = config.subagent.enabled;
    options.max_subagent_depth = config.subagent.max_depth;
    options.max_concurrent_subagents = config.subagent.max_concurrent;
    options.max_children_per_agent = config.subagent.max_children_per_agent;

    // Wire up tool profiles
    options.tool_profiles = config.subagent.tool_profiles.clone();

    // Register custom agent types from TOML config
    for def_cfg in &config.subagent.agents {
        options.subagents.push(synaptic_deep::SubAgentDef {
            name: def_cfg.name.clone(),
            description: def_cfg.description.clone(),
            system_prompt: def_cfg.system_prompt.clone(),
            tools: Vec::new(),
            model: None,
            tool_allow: def_cfg.tool_allow.clone(),
            tool_deny: def_cfg.tool_deny.clone(),
            timeout_secs: def_cfg.timeout_secs,
            max_turns: def_cfg.max_turns,
            tool_profile: def_cfg.tool_profile.clone(),
            permission_mode: def_cfg.permission_mode.clone(),
            skills: def_cfg.skills.clone(),
            background: def_cfg.background,
            hooks: None,
            memory: None,
        });
    }

    // Discover agents from .claude/agents/ directories
    let discovered_agents = discover_agents(cwd);
    for agent_def in discovered_agents {
        let already_defined = options.subagents.iter().any(|a| a.name == agent_def.name);
        if !already_defined {
            options.subagents.push(agent_def);
        }
    }

    // Wire up per-skill overrides
    if !config.skill_overrides.is_empty() {
        for (name, ov_cfg) in &config.skill_overrides {
            options.skill_overrides.insert(
                name.clone(),
                synaptic_deep::SkillOverride {
                    enabled: Some(ov_cfg.enabled),
                    env: ov_cfg.env.clone(),
                },
            );
        }
    }

    if let Some(max_turns) = config.base.agent.max_turns {
        options.max_input_tokens = max_turns * 4000;
    }

    // Add MCP tools
    options.tools.extend(mcp_tools);

    // Add apply_patch tool
    options
        .tools
        .push(crate::tools::ApplyPatchTool::new(cwd));

    // Add PDF reading tool
    options.tools.push(crate::tools::ReadPdfTool::new(cwd));

    // Add Firecrawl web scraping tool
    options
        .tools
        .push(crate::tools::FirecrawlTool::new());

    // Add image analysis tool (always available — works with any vision model)
    options
        .tools
        .push(crate::tools::AnalyzeImageTool::new(cwd));

    // Add audio transcription tool
    #[cfg(feature = "voice")]
    {
        // Try to create an OpenAI STT provider from environment
        if let Ok(voice) = synaptic_voice::openai::OpenAiVoice::new("OPENAI_API_KEY") {
            let stt: Arc<dyn synaptic_voice::SttProvider> = Arc::new(voice);
            options
                .tools
                .push(crate::tools::TranscribeAudioTool::new(cwd, stt));
            tracing::info!("Audio transcription tool registered");
        }
    }

    // Add memory tools if LTM is available
    if let Some(ref ltm) = ltm {
        options
            .tools
            .push(crate::tools::MemorySearchTool::new(ltm.clone()));
        options
            .tools
            .push(crate::tools::MemoryGetTool::new(ltm.clone()));
        tracing::info!("Memory tools registered (memory_search, memory_get)");
    }

    // Add session tools if SessionManager is available
    if let Some(ref mgr) = session_mgr {
        options
            .tools
            .push(crate::tools::SessionsListTool::new(mgr.clone()));
        options
            .tools
            .push(crate::tools::SessionsHistoryTool::new(mgr.clone()));
        options
            .tools
            .push(crate::tools::SessionsSendTool::new(mgr.clone()));
        options
            .tools
            .push(crate::tools::SessionsSpawnTool::new(mgr.clone()));
    }

    // Add browser tools if enabled
    #[cfg(feature = "browser")]
    {
        use synaptic::browser::{browser_tools, BrowserConfig};
        let browser_config = BrowserConfig::default();
        let tools = browser_tools(&browser_config);
        tracing::info!(tool_count = tools.len(), "Browser tools available");
        options.tools.extend(tools);
    }

    // --- Middleware stack ---

    // Agent tracing middleware (first, so it captures the full picture)
    options
        .middleware
        .push(Arc::new(AgentTracingMiddleware::new()));

    // Secret masking middleware
    if config.secrets.as_ref().is_none_or(|s| s.mask_api_keys) {
        let registry = Arc::new(SecretRegistry::new());

        if let Ok(api_key) = config.base.resolve_api_key() {
            if !api_key.is_empty() {
                registry.register("api_key", &api_key);
            }
        }

        if let Some(ref secrets_cfg) = config.secrets {
            for var_name in &secrets_cfg.additional_env_vars {
                if let Ok(val) = std::env::var(var_name) {
                    if !val.is_empty() {
                        registry.register(var_name, &val);
                    }
                }
            }
        }

        options
            .middleware
            .push(Arc::new(SecretMaskingMiddleware::new(registry)));
        tracing::info!("Secret masking enabled");
    }

    // SSRF guard middleware
    if config.security.as_ref().is_none_or(|s| s.ssrf_guard) {
        options
            .middleware
            .push(Arc::new(SsrfGuardMiddleware::new(SsrfGuardConfig::default())));
        tracing::info!("SSRF guard enabled");
    }

    // Security middleware
    if config.security.as_ref().is_none_or(|s| s.enabled) {
        let mut analyzer = RuleBasedAnalyzer::new()
            .with_default_risk(RiskLevel::Low)
            .with_tool_risk("bash", RiskLevel::High)
            .with_tool_risk("shell_exec", RiskLevel::High)
            .with_tool_risk("write_file", RiskLevel::Medium)
            .with_tool_risk("delete_file", RiskLevel::High);

        if let Some(ref sec_cfg) = config.security {
            for tool in &sec_cfg.high_risk_tools {
                analyzer = analyzer.with_tool_risk(tool.as_str(), RiskLevel::High);
            }
            for tool in &sec_cfg.blocked_tools {
                analyzer = analyzer.with_tool_risk(tool.as_str(), RiskLevel::Critical);
            }
        }

        let policy = Arc::new(ThresholdConfirmationPolicy::new(RiskLevel::High));
        let callback: Arc<dyn SecurityConfirmationCallback> = security_callback
            .clone()
            .unwrap_or_else(|| Arc::new(AutoApproveCallback));
        options.middleware.push(Arc::new(SecurityMiddleware::new(
            Arc::new(analyzer),
            policy,
            callback,
        )));
        tracing::info!("Security middleware enabled");
    }

    // Tool policy middleware
    {
        let tp = &config.tool_policy;
        let has_policy = !tp.owner_only_tools.is_empty()
            || !tp.tool_allow.is_empty()
            || !tp.tool_deny.is_empty();
        if has_policy {
            options
                .middleware
                .push(Arc::new(super::tool_policy::ToolPolicyMiddleware::new(
                    tp.clone(),
                )));
            tracing::info!("Tool policy middleware enabled");
        }
    }

    // Circuit breaker middleware
    {
        let threshold = config
            .security
            .as_ref()
            .map(|s| s.circuit_breaker_threshold)
            .unwrap_or(5);
        let cb_config = CircuitBreakerConfig {
            failure_threshold: threshold,
            ..Default::default()
        };
        options
            .middleware
            .push(Arc::new(CircuitBreakerMiddleware::new(cb_config)));
        tracing::info!(threshold = threshold, "Circuit breaker enabled");
    }

    // Loop detection middleware
    {
        let max_repeats = 3;
        options
            .middleware
            .push(Arc::new(LoopDetectionMiddleware::new(max_repeats)));
        tracing::info!(threshold = max_repeats, "Loop detection enabled");
    }

    // Thinking middleware (session override or config default)
    {
        let overrides = session_overrides.as_ref();
        let thinking_level = overrides
            .and_then(|o| o.thinking.as_deref())
            .or_else(|| {
                // Check model entry for default thinking level
                if let Some(ref models) = config.model_catalog {
                    let primary = &config.base.model.model;
                    models
                        .iter()
                        .find(|m| m.name == *primary || m.aliases.contains(&primary.to_string()))
                        .and_then(|m| m.thinking.as_deref())
                } else {
                    None
                }
            });

        if let Some(level) = thinking_level {
            if level == "adaptive" {
                options
                    .middleware
                    .push(Arc::new(ThinkingMiddleware::adaptive()));
                tracing::info!("Adaptive thinking enabled");
            } else if let Some(tc) = super::thinking::parse_thinking_level(level) {
                options
                    .middleware
                    .push(Arc::new(ThinkingMiddleware::new(Some(tc))));
                tracing::info!(level = %level, "Thinking mode enabled");
            }
        }
    }

    // Verbose middleware (session override)
    {
        let verbose_level = session_overrides
            .as_ref()
            .and_then(|o| o.verbose.as_deref());
        if let Some(level) = verbose_level {
            if level != "on" && level != "inherit" {
                options
                    .middleware
                    .push(Arc::new(VerboseMiddleware::new(level)));
                tracing::info!(level = %level, "Verbose mode enabled");
            }
        }
    }

    // Auto-compaction via CondenserMiddleware
    if config.memory.auto_compact {
        let condenser: Arc<dyn Condenser> = match config.memory.compact_strategy.as_str() {
            "summarize" => Arc::new(LlmSummarizingCondenser::new(
                model.clone(),
                config.memory.auto_compact_threshold,
                config.memory.keep_recent,
            )),
            "chunked" => Arc::new(ChunkedSummarizingCondenser::new(
                model.clone(),
                config.memory.auto_compact_threshold,
                config.memory.keep_recent,
                30,
            )),
            _ => {
                let counter: Arc<dyn TokenCounter> =
                    Arc::new(synaptic::core::HeuristicTokenCounter);
                Arc::new(TokenBudgetCondenser::new(
                    config.memory.auto_compact_threshold,
                    counter,
                ))
            }
        };
        options
            .middleware
            .push(Arc::new(CondenserMiddleware::new(condenser)));
        tracing::info!(
            strategy = %config.memory.compact_strategy,
            threshold = config.memory.auto_compact_threshold,
            "Auto-compaction enabled"
        );
    }

    // DeepSummarizationMiddleware
    if config.memory.auto_compact {
        let summarization = synaptic_deep::middleware::summarization::DeepSummarizationMiddleware::new(
            backend.clone(),
            model.clone(),
            config.memory.auto_compact_threshold,
            0.8,
        );
        options.middleware.push(Arc::new(summarization));
        tracing::info!("Deep summarization middleware enabled");
    }

    // Cost tracking middleware — records token usage from every LLM response
    if let Some(tracker) = cost_tracker {
        let model_name = config.base.model.model.clone();
        tracker.set_model(&model_name).await;
        options
            .middleware
            .push(Arc::new(CostTrackingMw(tracker)));
        tracing::info!("Cost tracking middleware enabled");
    }

    // OpenTelemetry callback middleware
    #[cfg(feature = "otel")]
    {
        if std::env::var("OTEL_EXPORTER_OTLP_ENDPOINT").is_ok() {
            use synaptic::callbacks::OpenTelemetryCallback;
            use synaptic::middleware::CallbackMiddleware;
            let otel_cb = Arc::new(OpenTelemetryCallback::new("synapse"));
            options
                .middleware
                .push(Arc::new(CallbackMiddleware::new(otel_cb)));
            tracing::info!("OTel callback middleware injected");
        }
    }

    // Fallback middleware
    if let Some(fallback_mw) = build_fallback_middleware(config) {
        options.middleware.push(Arc::new(fallback_mw));
    }

    // Skill hooks executor
    options.hooks_executor = Some(Arc::new(
        crate::hooks::SynapseHooksExecutor::new(Arc::new(config.clone())),
    ));

    create_deep_agent(model, options)
}
