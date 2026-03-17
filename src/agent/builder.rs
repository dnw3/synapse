use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;

use async_trait::async_trait;
use synaptic::callbacks::CostTrackingCallback;
use synaptic::condenser::{
    ChunkedSummarizingCondenser, Condenser, CondenserMiddleware, LlmSummarizingCondenser,
    TokenBudgetCondenser,
};
use synaptic::core::{ChatModel, SynapticError, TokenCounter, Tool};
use synaptic::deep::{create_deep_agent, DeepAgentOptions};
use synaptic::events::EventBus;
use synaptic::graph::{Checkpointer, CompiledGraph, MessageState};
use synaptic::middleware::{
    CircuitBreakerConfig, CircuitBreakerMiddleware, RiskLevel, RuleBasedAnalyzer,
    SecurityConfirmationCallback, SecurityMiddleware, SsrfGuardConfig, SsrfGuardMiddleware,
    ThresholdConfirmationPolicy,
};
use synaptic::secrets::{SecretMaskingMiddleware, SecretRegistry};
use synaptic_deep::backend::FilesystemBackend;
use synaptic_deep::CommandExecutor;

use crate::config::SynapseConfig;

use super::callbacks::AutoApproveCallback;
use super::context::load_project_context;
use super::discovery::discover_agents;
use super::middleware::{build_fallback_middleware, LoopDetectionMiddleware};
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
        "unknown",
        None,
        None,
        None,
        None,
    )
    .await
}

/// Build a Deep Agent with an optional custom security callback and optional LTM.
#[allow(clippy::too_many_arguments)]
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
    channel: &str,
    agent_name: Option<&str>,
    event_bus: Option<Arc<EventBus>>,
    plugin_registry: Option<Arc<RwLock<synaptic::plugin::PluginRegistry>>>,
    channel_registry: Option<Arc<tokio::sync::RwLock<crate::gateway::messages::ChannelRegistry>>>,
) -> Result<CompiledGraph<MessageState>, SynapticError> {
    // Use docker backend if configured, otherwise filesystem
    #[cfg(feature = "docker")]
    let backend: Arc<dyn synaptic_deep::backend::Backend> = {
        if let Some(ref docker_cfg) = config.docker {
            if docker_cfg.enabled {
                let image = docker_cfg.image.as_deref().unwrap_or("ubuntu:22.04");
                let work_dir = cwd.to_string_lossy();
                tracing::info!(backend = "docker", image = %image, "Creating Docker container");
                match crate::docker::manager::DockerManager::create_workspace_with_mount(
                    image, &work_dir, &work_dir,
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
    let backend: Arc<dyn synaptic_deep::backend::Backend> = Arc::new(FilesystemBackend::new(cwd));

    let mut options = DeepAgentOptions::new(backend.clone());

    // System prompt + project context
    let mut system_prompt = system_prompt_override
        .map(|s| s.to_string())
        .or_else(|| config.base.agent.system_prompt.clone())
        .unwrap_or_else(|| {
            "You are Synapse, a helpful AI assistant powered by the Synaptic framework.".to_string()
        });

    let workspace_dir = config.workspace_dir_for_agent(agent_name);
    let project_context = load_project_context(&workspace_dir, cwd, &config.context);
    if !project_context.is_empty() {
        system_prompt.push_str("\n\n# Project Context\n\n");
        system_prompt.push_str(&project_context);
    }

    options.system_prompt = Some(system_prompt);

    // Build the memory provider once — used for profile injection and the
    // memory_search tool below.
    let memory_provider_arc = crate::memory::build_memory_provider(config, ltm.clone());

    // Inject user profile from memory provider (if available)
    {
        let user_id = if channel == "web" || channel == "unknown" {
            "default"
        } else {
            channel
        };
        match memory_provider_arc.get_profile(user_id).await {
            Ok(Some(profile)) if !profile.is_empty() => {
                let base = options.system_prompt.as_deref().unwrap_or("").to_string();
                options.system_prompt = Some(format!("{}\n\n## User Profile\n{}", base, profile));
                tracing::debug!(user_id, "injected user profile into system prompt");
            }
            _ => {} // No profile or error — skip silently
        }
    }

    // Self-awareness: environment detection + agent identity / self-modification guide
    {
        use synaptic_deep::{ChannelInfo, EnvironmentInfo};
        let mut env = EnvironmentInfo::detect();
        // Enrich with git info
        env.git_root = std::process::Command::new("git")
            .args(["rev-parse", "--show-toplevel"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
        env.git_branch = std::process::Command::new("git")
            .args(["rev-parse", "--abbrev-ref", "HEAD"])
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_string());
        // Channel info
        let (caps, limit) = super::self_awareness::channel_info_for(channel);
        env.channel = Some(ChannelInfo {
            name: channel.to_string(),
            capabilities: caps.iter().map(|s| s.to_string()).collect(),
            message_limit: limit,
        });
        options.environment = Some(env);
        options.self_section = Some(super::self_awareness::build_self_section(config, channel));
    }
    options.enable_filesystem = config.base.agent.tools.filesystem;
    options.memory_file = Some(config.base.paths.memory_file.clone());

    // Build skills_dirs: synapse personal > claude personal > project > legacy > custom config
    {
        let mut skills_dirs = Vec::new();
        if let Some(home) = dirs::home_dir() {
            // Synapse's own personal skills (highest priority)
            skills_dirs.push(home.join(".synapse/skills").to_string_lossy().to_string());
            // OpenClaw-compatible personal skills (fallback)
            skills_dirs.push(home.join(".claude/skills").to_string_lossy().to_string());
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
        // Add extra skill directories from config
        for extra in &config.skills.extra_dirs {
            let expanded = if extra.starts_with("~/") {
                dirs::home_dir()
                    .unwrap_or_default()
                    .join(extra.trim_start_matches("~/"))
                    .to_string_lossy()
                    .to_string()
            } else {
                extra.clone()
            };
            skills_dirs.push(expanded);
        }
        options.skills_dirs = skills_dirs;
    }

    // Use configured prompt budget for skill descriptions
    options.skill_description_budget = config.skills.max_skills_prompt_chars;

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

    // Add plugin-registered tools
    if let Some(ref registry) = plugin_registry {
        let reg = registry.read().unwrap();
        for tool in reg.tools() {
            options.tools.push(tool.clone());
        }
        tracing::debug!(count = reg.tools().len(), "Plugin tools merged into agent");
    }

    // Add apply_patch tool
    options.tools.push(crate::tools::ApplyPatchTool::new(cwd));

    // Add PDF reading tool
    options.tools.push(crate::tools::ReadPdfTool::new(cwd));

    // Add Firecrawl web scraping tool
    options.tools.push(crate::tools::FirecrawlTool::new());

    // Add image analysis tool (always available — works with any vision model)
    options.tools.push(crate::tools::AnalyzeImageTool::new(cwd));

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

    // Add memory tools — memory_search uses the configured MemoryProvider,
    // memory_get uses LTM directly (count/list are LTM-specific).
    {
        options.tools.push(crate::tools::MemorySearchTool::new(
            memory_provider_arc.clone(),
        ));
        tracing::info!("memory_search tool registered");
    }
    if let Some(ref ltm) = ltm {
        options
            .tools
            .push(crate::tools::MemoryGetTool::new(ltm.clone()));
        tracing::info!("memory_get tool registered");
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

    // Add platform action tool (channel registry wired when running as gateway)
    {
        let tool = if let Some(ref reg) = channel_registry {
            crate::tools::PlatformActionTool::with_registry(reg.clone())
        } else {
            crate::tools::PlatformActionTool::new()
        };
        options.tools.push(Arc::new(tool));
        tracing::debug!(
            has_registry = channel_registry.is_some(),
            "PlatformActionTool registered"
        );
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
        options.middleware.push(Arc::new(SsrfGuardMiddleware::new(
            SsrfGuardConfig::default(),
        )));
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
        let thinking_level = overrides.and_then(|o| o.thinking.as_deref()).or_else(|| {
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
        let summarization =
            synaptic_deep::middleware::summarization::DeepSummarizationMiddleware::new(
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
        options.middleware.push(Arc::new(CostTrackingMw(tracker)));
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
    options.hooks_executor = Some(Arc::new(crate::hooks::SynapseHooksExecutor::new(Arc::new(
        config.clone(),
    ))));

    // Post-session reflection for self-evolution
    if config.reflection.enabled {
        let reflection_model_name = config.reflection.model.as_deref();
        match reflection_model_name {
            Some(name) => match super::model::build_model_by_name(config, name) {
                Ok(m) => {
                    let rcfg = synaptic_deep::ReflectionConfig {
                        min_messages: config.reflection.min_messages,
                        memory_file: config.base.paths.memory_file.clone(),
                        ..Default::default()
                    };
                    options.reflection_model = Some(m);
                    options.reflection_config = Some(rcfg);
                    tracing::info!(model = name, "Reflection middleware enabled");
                }
                Err(e) => tracing::warn!(error = %e, "Failed to build reflection model, skipping"),
            },
            None => {
                // Use main model (fallback)
                let rcfg = synaptic_deep::ReflectionConfig {
                    min_messages: config.reflection.min_messages,
                    memory_file: config.base.paths.memory_file.clone(),
                    ..Default::default()
                };
                options.reflection_model = Some(model.clone());
                options.reflection_config = Some(rcfg);
                tracing::info!("Reflection middleware enabled (using main model)");
            }
        }
    }

    // Wire EventBus into deep agent for lifecycle event emission
    options.event_bus = event_bus;
    options.model_name = Some(config.base.model.model.clone());
    options.provider_name = Some(config.base.model.provider.clone());
    options.channel = Some(channel.to_string());
    options.agent_id = agent_name.map(|s| s.to_string());

    create_deep_agent(model, options)
}
