use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use synaptic::callbacks::CostTrackingCallback;
use synaptic::core::{ChatModel, SynapticError, Tool};
use synaptic::deep::backend::FilesystemBackend;
use synaptic::deep::CommandExecutor;
use synaptic::deep::{create_deep_agent, DeepAgentOptions};
use synaptic::events::EventBus;
use synaptic::graph::{Checkpointer, CompiledGraph, MessageState};
use synaptic::middleware::SecurityConfirmationCallback;

use crate::config::SynapseConfig;

use super::bootstrap::{BootstrapLoader, SessionKind};
use super::discovery::discover_agents;
use super::middleware_setup;
use super::tools_setup;

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
    session_kind: SessionKind,
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
        "unknown",
        None,
        None,
        None,
        None,
        session_kind,
        &[],
    )
    .await
}

/// Build a Deep Agent with an optional custom security callback.
#[allow(clippy::too_many_arguments)]
pub async fn build_deep_agent_with_callback(
    model: Arc<dyn ChatModel>,
    config: &SynapseConfig,
    cwd: &Path,
    checkpointer: Arc<dyn Checkpointer>,
    mcp_tools: Vec<Arc<dyn Tool>>,
    system_prompt_override: Option<&str>,
    security_callback: Option<Arc<dyn SecurityConfirmationCallback>>,
    session_mgr: Option<Arc<synaptic::session::SessionManager>>,
    session_overrides: Option<SessionOverrides>,
    cost_tracker: Option<Arc<CostTrackingCallback>>,
    channel: &str,
    agent_name: Option<&str>,
    event_bus: Option<Arc<EventBus>>,
    plugin_registry: Option<Arc<tokio::sync::RwLock<synaptic::plugin::PluginRegistry>>>,
    channel_registry: Option<Arc<tokio::sync::RwLock<crate::gateway::messages::ChannelRegistry>>>,
    session_kind: SessionKind,
    extra_skills_dirs: &[std::path::PathBuf],
) -> Result<CompiledGraph<MessageState>, SynapticError> {
    // --- Backend selection ---
    #[cfg(feature = "sandbox")]
    let backend: Arc<dyn synaptic::deep::backend::Backend> = {
        use crate::sandbox::orchestrator::{ResolvedBackend, SandboxOrchestrator};

        let orchestrator: Option<Arc<SandboxOrchestrator>> =
            if let Some(ref sandbox_cfg) = config.sandbox {
                if sandbox_cfg.mode != crate::sandbox::config::SandboxMode::Off {
                    use synaptic::deep::sandbox::SandboxProviderRegistry;
                    let provider_registry = Arc::new(SandboxProviderRegistry::new());
                    let persistent = crate::sandbox::registry::SandboxPersistentRegistry::new(
                        crate::sandbox::registry::SandboxPersistentRegistry::default_path(),
                    );
                    Some(Arc::new(SandboxOrchestrator::new(
                        provider_registry,
                        sandbox_cfg.clone(),
                        persistent,
                    )))
                } else {
                    None
                }
            } else {
                None
            };

        if let Some(ref orch) = orchestrator {
            let session_key = "main";
            let agent_id = agent_name.unwrap_or("default");
            match orch.resolve_backend(session_key, agent_id, None).await? {
                ResolvedBackend::Host => Arc::new(FilesystemBackend::new(cwd)),
                ResolvedBackend::Sandboxed(instance) => {
                    tracing::info!(
                        backend = "sandbox",
                        runtime_id = %instance.runtime_id,
                        "Using sandboxed backend"
                    );
                    instance.backend.clone()
                }
            }
        } else {
            Arc::new(FilesystemBackend::new(cwd))
        }
    };
    #[cfg(all(not(feature = "sandbox"), feature = "docker"))]
    let backend: Arc<dyn synaptic::deep::backend::Backend> = {
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
    #[cfg(not(any(feature = "sandbox", feature = "docker")))]
    let backend: Arc<dyn synaptic::deep::backend::Backend> = Arc::new(FilesystemBackend::new(cwd));

    let mut options = DeepAgentOptions::new(backend.clone());
    options.filesystem.path_guard = Some(Arc::new(
        synaptic::deep::tools::path_guard::PathGuard::new(cwd.to_path_buf()),
    ));

    // --- System prompt + project context ---
    let mut system_prompt = system_prompt_override
        .map(|s| s.to_string())
        .or_else(|| config.agent_config().system_prompt.clone())
        .unwrap_or_else(|| {
            "You are Synapse, a helpful AI assistant powered by the Synaptic framework.".to_string()
        });

    let workspace_dir = config.workspace_dir_for_agent(agent_name);
    let loader = BootstrapLoader::new(workspace_dir, config.context.clone());
    let bootstrap_files = loader.load(session_kind);
    let bootstrap_context = BootstrapLoader::format_for_prompt(&bootstrap_files);
    if !bootstrap_context.is_empty() {
        system_prompt.push_str("\n\n");
        system_prompt.push_str(&bootstrap_context);
    }

    // Migration: warn if CWD has CLAUDE.md but it's not configured as extra_file
    if config.context.extra_files.is_empty() && config.context.extra_patterns.is_empty() {
        let claude_md = cwd.join("CLAUDE.md");
        if claude_md.exists() {
            tracing::warn!(
                path = %claude_md.display(),
                "CLAUDE.md found in CWD but not loaded. \
                 Add it to [context].extra_files in synapse.toml if needed."
            );
        }
    }

    options.context.system_prompt = Some(system_prompt);

    // --- Memory provider + user profile ---
    // Get memory provider from plugin registry (set by memory plugin during plugin init)
    let memory_provider_arc: Arc<dyn synaptic::memory::MemoryProvider> = {
        if let Some(ref registry) = plugin_registry {
            let reg = registry.read().await;
            if let Some(provider) = reg.memory_slot() {
                provider.clone()
            } else {
                tracing::warn!("no memory plugin registered, using noop provider");
                Arc::new(crate::memory::NativeMemoryProvider::new_noop())
            }
        } else {
            // No plugin registry at all — noop memory (callers should use build_cli_plugins)
            tracing::warn!("no plugin registry provided, using noop memory provider");
            Arc::new(crate::memory::NativeMemoryProvider::new_noop())
        }
    };

    // Inject user profile from memory provider (if available)
    {
        let user_id = if channel == "web" || channel == "unknown" {
            "default"
        } else {
            channel
        };
        match memory_provider_arc.get_profile(user_id).await {
            Ok(Some(profile)) if !profile.is_empty() => {
                let base = options
                    .context
                    .system_prompt
                    .as_deref()
                    .unwrap_or("")
                    .to_string();
                options.context.system_prompt =
                    Some(format!("{}\n\n## User Profile\n{}", base, profile));
                tracing::debug!(user_id, "injected user profile into system prompt");
            }
            _ => {} // No profile or error — skip silently
        }
    }

    // --- Self-awareness: environment detection + agent identity ---
    {
        use synaptic::deep::{ChannelInfo, EnvironmentInfo};
        let mut env = EnvironmentInfo::detect();
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
        let (caps, limit) = super::self_awareness::channel_info_for(channel);
        env.channel = Some(ChannelInfo {
            name: channel.to_string(),
            capabilities: caps.iter().map(|s| s.to_string()).collect(),
            message_limit: limit,
        });
        options.context.environment = Some(env);
        options.context.self_section =
            Some(super::self_awareness::build_self_section(config, channel));
    }

    options.filesystem.enable_filesystem = config.agent_config().tools.filesystem;
    // Disable DeepMemoryMiddleware — bootstrap context (AGENTS.md etc.) is already
    // injected once at startup via BootstrapLoader. Re-reading on every model call
    // would cause double injection. Memory access at runtime goes through tools instead.
    options.context.enable_memory = false;
    options.context.memory_file = Some(config.memory_file().to_string());

    // --- Skills directories ---
    setup_skills_dirs(&mut options, config, cwd, agent_name);
    // Append bundle-contributed skills dirs (from plugin ecosystem)
    for dir in extra_skills_dirs {
        options
            .skills
            .skills_dirs
            .push(dir.to_string_lossy().to_string());
    }

    options.skills.skill_description_budget = config.skills.max_skills_prompt_chars;
    options.skills.command_executor = Some(Arc::new(ShellCommandExecutor::new(cwd)));
    options.checkpointer = Some(checkpointer);

    // --- Subagent config ---
    setup_subagents(&mut options, config, cwd);

    // --- Per-skill overrides ---
    if !config.skill_overrides.is_empty() {
        for (name, ov_cfg) in &config.skill_overrides {
            options.skills.skill_overrides.insert(
                name.clone(),
                synaptic::deep::SkillOverride {
                    enabled: Some(ov_cfg.enabled),
                    env: ov_cfg.env.clone(),
                },
            );
        }
    }

    if let Some(max_turns) = config.agent_config().max_turns {
        options.condenser.max_input_tokens = max_turns * 4000;
    }

    // --- Tools ---
    tools_setup::register_tools(
        &mut options,
        cwd,
        mcp_tools,
        session_mgr.as_ref(),
        plugin_registry.as_ref(),
        channel_registry.as_ref(),
    )
    .await;

    // --- Middleware stack ---
    middleware_setup::setup_middleware(
        &mut options,
        config,
        &model,
        &backend,
        security_callback,
        session_overrides.as_ref(),
        cost_tracker,
    )
    .await;

    // --- Hooks executor ---
    options.skills.hooks_executor = Some(Arc::new(crate::hooks::SynapseHooksExecutor::new(
        Arc::new(config.clone()),
    )));

    // --- Reflection ---
    middleware_setup::setup_reflection(&mut options, config, &model);

    // --- EventBus + metadata ---
    options.observability.event_bus = event_bus;
    options.observability.model_name = Some(config.model_config().model.clone());
    options.observability.provider_name = Some(config.model_config().provider.clone());
    options.observability.channel = Some(channel.to_string());
    options.observability.agent_id = agent_name.map(|s| s.to_string());

    // Plugin hook interceptor — bridges EventBus lifecycle events to plugin subscribers
    if let Some(ref bus) = options.observability.event_bus {
        options
            .interceptors
            .push(Arc::new(synaptic::middleware::PluginHookInterceptor::new(
                bus.clone(),
            )));
    }

    // Inject plugin-registered interceptors
    if let Some(ref registry) = plugin_registry {
        let reg = registry.read().await;
        for interceptor in reg.interceptors() {
            options.interceptors.push(Arc::clone(interceptor));
        }
    }

    create_deep_agent(model, options)
}

/// Build skills_dirs with OpenClaw-aligned precedence:
///   1. <workspace>/skills/   — per-agent (highest priority)
///   2. ~/.synapse/skills/    — global shared
///   3. config extra_dirs     — custom paths
///
/// Bundle-contributed skills dirs are added separately by PluginManager.
fn setup_skills_dirs(
    options: &mut DeepAgentOptions,
    config: &SynapseConfig,
    cwd: &Path,
    agent_name: Option<&str>,
) {
    let mut skills_dirs = Vec::new();

    // 1. Per-agent workspace skills (highest priority)
    let workspace_dir = config.workspace_dir_for_agent(agent_name);
    let agent_skills = workspace_dir.join("skills");
    skills_dirs.push(agent_skills.to_string_lossy().to_string());

    // 2. Global shared skills
    if let Some(home) = dirs::home_dir() {
        skills_dirs.push(home.join(".synapse/skills").to_string_lossy().to_string());
    }

    // 3. Custom extra dirs from config
    for extra in &config.skills.extra_dirs {
        let expanded = if extra.starts_with("~/") {
            dirs::home_dir()
                .unwrap_or_default()
                .join(extra.trim_start_matches("~/"))
                .to_string_lossy()
                .to_string()
        } else if std::path::Path::new(extra).is_absolute() {
            extra.clone()
        } else {
            cwd.join(extra).to_string_lossy().to_string()
        };
        skills_dirs.push(expanded);
    }

    options.skills.skills_dirs = skills_dirs;
}

/// Wire up subagent config: tool profiles, agent types, discovered agents.
fn setup_subagents(options: &mut DeepAgentOptions, config: &SynapseConfig, cwd: &Path) {
    options.subagent.enable_subagents = config.subagent.enabled;
    options.subagent.max_subagent_depth = config.subagent.max_depth;
    options.subagent.max_concurrent_subagents = config.subagent.max_concurrent;
    options.subagent.max_children_per_agent = config.subagent.max_children_per_agent;
    options.subagent.tool_profiles = config.subagent.tool_profiles.clone();

    // Register custom agent types from TOML config
    for def_cfg in &config.subagent.agents {
        options
            .subagent
            .subagents
            .push(synaptic::deep::SubAgentDef {
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
        let already_defined = options
            .subagent
            .subagents
            .iter()
            .any(|a| a.name == agent_def.name);
        if !already_defined {
            options.subagent.subagents.push(agent_def);
        }
    }
}
