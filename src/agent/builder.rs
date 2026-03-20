use std::path::Path;
use std::sync::Arc;
use std::sync::RwLock;

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

use super::context::load_project_context;
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
    // --- Backend selection ---
    #[cfg(feature = "docker")]
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
    #[cfg(not(feature = "docker"))]
    let backend: Arc<dyn synaptic::deep::backend::Backend> = Arc::new(FilesystemBackend::new(cwd));

    let mut options = DeepAgentOptions::new(backend.clone());

    // --- System prompt + project context ---
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

    // --- Memory provider + user profile ---
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
        options.environment = Some(env);
        options.self_section = Some(super::self_awareness::build_self_section(config, channel));
    }

    options.enable_filesystem = config.base.agent.tools.filesystem;
    options.memory_file = Some(config.base.paths.memory_file.clone());

    // --- Skills directories ---
    setup_skills_dirs(&mut options, config, cwd);

    options.skill_description_budget = config.skills.max_skills_prompt_chars;
    options.command_executor = Some(Arc::new(ShellCommandExecutor::new(cwd)));
    options.checkpointer = Some(checkpointer);

    // --- Subagent config ---
    setup_subagents(&mut options, config, cwd);

    // --- Per-skill overrides ---
    if !config.skill_overrides.is_empty() {
        for (name, ov_cfg) in &config.skill_overrides {
            options.skill_overrides.insert(
                name.clone(),
                synaptic::deep::SkillOverride {
                    enabled: Some(ov_cfg.enabled),
                    env: ov_cfg.env.clone(),
                },
            );
        }
    }

    if let Some(max_turns) = config.base.agent.max_turns {
        options.max_input_tokens = max_turns * 4000;
    }

    // --- Tools ---
    tools_setup::register_tools(
        &mut options,
        cwd,
        mcp_tools,
        memory_provider_arc,
        ltm.as_ref(),
        session_mgr.as_ref(),
        plugin_registry.as_ref(),
        channel_registry.as_ref(),
    );

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
    options.hooks_executor = Some(Arc::new(crate::hooks::SynapseHooksExecutor::new(Arc::new(
        config.clone(),
    ))));

    // --- Reflection ---
    middleware_setup::setup_reflection(&mut options, config, &model);

    // --- EventBus + metadata ---
    options.event_bus = event_bus;
    options.model_name = Some(config.base.model.model.clone());
    options.provider_name = Some(config.base.model.provider.clone());
    options.channel = Some(channel.to_string());
    options.agent_id = agent_name.map(|s| s.to_string());

    create_deep_agent(model, options)
}

/// Build skills_dirs: synapse personal > claude personal > project > legacy > custom config.
fn setup_skills_dirs(options: &mut DeepAgentOptions, config: &SynapseConfig, cwd: &Path) {
    let mut skills_dirs = Vec::new();
    if let Some(home) = dirs::home_dir() {
        skills_dirs.push(home.join(".synapse/skills").to_string_lossy().to_string());
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

/// Wire up subagent config: tool profiles, agent types, discovered agents.
fn setup_subagents(options: &mut DeepAgentOptions, config: &SynapseConfig, cwd: &Path) {
    options.enable_subagents = config.subagent.enabled;
    options.max_subagent_depth = config.subagent.max_depth;
    options.max_concurrent_subagents = config.subagent.max_concurrent;
    options.max_children_per_agent = config.subagent.max_children_per_agent;
    options.tool_profiles = config.subagent.tool_profiles.clone();

    // Register custom agent types from TOML config
    for def_cfg in &config.subagent.agents {
        options.subagents.push(synaptic::deep::SubAgentDef {
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
}
