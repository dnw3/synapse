// Framework middleware deprecated → EventSubscriber migration in progress
#![allow(deprecated)]

#[cfg(feature = "web")]
mod acp;
mod agent;
#[cfg(feature = "broadcast")]
mod broadcast;
mod cli;
mod commands;
mod config;
pub mod cron;
mod display;
mod doctor;
mod heartbeat;
mod hooks;
mod hub;
mod init;
mod logging;
mod memory;
mod notify;
#[allow(dead_code)]
mod plugin;
#[allow(dead_code)]
mod plugin_loader;
mod repl;
#[allow(dead_code)]
mod router;
mod scheduler;
mod service;
mod session;
mod task;
mod tools;
mod tunnel;
mod usage;
mod voice;
mod workflow;

#[cfg(feature = "otel")]
mod otel;

#[cfg(feature = "web")]
mod gateway;

#[cfg(any(
    feature = "bot-lark",
    feature = "bot-slack",
    feature = "bot-telegram",
    feature = "bot-discord",
    feature = "bot-dingtalk",
    feature = "bot-mattermost",
    feature = "bot-matrix",
    feature = "bot-teams",
    feature = "bot-whatsapp",
    feature = "bot-signal",
    feature = "bot-imessage",
    feature = "bot-line",
    feature = "bot-googlechat",
    feature = "bot-wechat",
    feature = "bot-irc",
))]
mod channels;

#[cfg(feature = "docker")]
mod docker;

#[cfg(feature = "tui")]
mod tui;

use std::path::PathBuf;
use std::sync::Arc;

use clap::Parser;
use colored::Colorize;
use synaptic::core::{MemoryStore, Message};
use synaptic::session::SessionManager;

use crate::cli::{Cli, Command};
use crate::config::SynapseConfig;

#[tokio::main]
async fn main() {
    // Load .env file if present (silently ignore if missing)
    let _ = dotenvy::dotenv();

    // Tracing is initialized after config loads (inside run()) so that
    // LogConfig drives console level, file logging, and memory buffer.
    // For pre-config errors we rely on eprintln.

    #[cfg(feature = "otel")]
    let _otel_provider = otel::init_otel();

    let cli = Cli::parse();

    if let Err(e) = run(cli).await {
        eprintln!("{} {}", "error:".red().bold(), e);
        std::process::exit(1);
    }
}

/// Re-export for backward compatibility with other modules that use `crate::build_session_manager`.
pub(crate) fn build_session_manager(config: &SynapseConfig) -> SessionManager {
    session::build_session_manager(config)
}

async fn run(cli: Cli) -> Result<(), Box<dyn std::error::Error>> {
    // Init doesn't need config
    if matches!(cli.command, Some(Command::Init)) {
        return init::run_init().await;
    }

    let config = SynapseConfig::load_or_default(cli.config_path.as_deref())?;

    // Initialize structured logging (console + optional file + in-memory buffer)
    let log_buffer = logging::init_tracing(&config.logging);

    // Clean up old log files on startup
    if config.logging.file.enabled {
        let log_dir = std::path::PathBuf::from(logging::expand_log_path(&config.logging.file.path));
        logging::cleanup_old_logs(
            &log_dir,
            config.logging.file.max_days,
            config.logging.file.max_files,
        );
    }

    // Start scheduler if configured (runs in background)
    let _scheduler = if config.schedules.as_ref().is_some_and(|s| !s.is_empty()) {
        let model = agent::build_model(&config, cli.model_override.as_deref())?;
        match scheduler::start_scheduler(&config, model).await {
            Ok(s) => {
                tracing::info!(
                    jobs = config.schedules.as_ref().map(|s| s.len()).unwrap_or(0),
                    "Scheduler started"
                );
                Some(s)
            }
            Err(e) => {
                tracing::warn!(error = %e, "Failed to start scheduler");
                None
            }
        }
    } else {
        None
    };

    // Start heartbeat runner if enabled (runs in background)
    let _heartbeat = if config.heartbeat.enabled {
        let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);
        let runner = heartbeat::HeartbeatRunner::new(config.heartbeat.clone(), shutdown_rx);
        let handle = runner.start();
        tracing::info!(
            interval = %config.heartbeat.interval,
            prompt = %config.heartbeat.prompt_file,
            "Heartbeat enabled"
        );
        Some((handle, shutdown_tx))
    } else {
        None
    };

    match cli.command {
        Some(Command::Init) => unreachable!(), // handled above
        #[cfg(feature = "web")]
        Some(Command::Qr {
            setup_code_only,
            url,
        }) => {
            commands::qr::run(&config, setup_code_only, url);
            Ok(())
        }
        #[cfg(feature = "web")]
        Some(Command::Devices {
            action,
            request_id,
            device,
            name,
        }) => {
            commands::devices::run(
                &action,
                &commands::devices::DevicesArgs {
                    request_id,
                    device_id: device,
                    name,
                },
            );
            Ok(())
        }
        #[cfg(feature = "web")]
        Some(Command::Pairing {
            action,
            channel,
            value,
        }) => {
            commands::pairing_cmd::run(&action, &channel, value.as_deref()).await;
            Ok(())
        }
        Some(Command::Doctor) => doctor::run_doctor(&config).await,
        Some(Command::Chat { message }) => {
            run_chat(
                &config,
                message.as_deref(),
                cli.session_id.as_deref(),
                cli.model_override.as_deref(),
            )
            .await
        }
        Some(Command::Task { message, cwd }) => {
            let model = agent::build_model(&config, cli.model_override.as_deref())?;
            task::run_task(
                &config,
                model,
                &message,
                cli.session_id.as_deref(),
                cwd.as_deref(),
            )
            .await
        }
        Some(Command::Sessions) => {
            let mgr = build_session_manager(&config);
            repl::list_sessions(&mgr).await
        }
        #[cfg(feature = "web")]
        Some(Command::Serve { host, port }) => {
            // Initialize workspace templates if needed
            let workspace_dir = config.workspace_dir();
            agent::workspace::initialize_workspace(&workspace_dir);

            let host = host
                .or_else(|| config.serve.as_ref().and_then(|s| s.host.clone()))
                .unwrap_or_else(|| "0.0.0.0".to_string());
            let port = port
                .or_else(|| config.serve.as_ref().and_then(|s| s.port))
                .unwrap_or(3000);
            gateway::run_server_with_log_buffer(&config, &host, port, Some(log_buffer.clone()))
                .await
        }
        #[cfg(any(
            feature = "bot-lark",
            feature = "bot-slack",
            feature = "bot-telegram",
            feature = "bot-discord",
            feature = "bot-dingtalk",
            feature = "bot-mattermost",
            feature = "bot-matrix",
            feature = "bot-teams",
            feature = "bot-whatsapp",
            feature = "bot-signal",
            feature = "bot-imessage",
            feature = "bot-line",
            feature = "bot-googlechat",
            feature = "bot-wechat",
            feature = "bot-irc",
        ))]
        Some(Command::Bot { platform }) => {
            channels::run_bot(&config, &platform, cli.model_override.as_deref()).await
        }
        Some(Command::Voice) => {
            let model = agent::build_model(&config, cli.model_override.as_deref())?;
            voice::run_voice_mode(model, config.voice.as_ref()).await
        }
        Some(Command::Tunnel {
            provider,
            port,
            remote_host,
        }) => tunnel::start_tunnel(&provider, port, remote_host.as_deref()).await,
        Some(Command::Skill { action, name }) => {
            commands::run_skill_command(&action, name.as_deref())
        }
        Some(Command::Plugin { action, name }) => {
            commands::run_plugin_command(&action, name.as_deref())
        }
        Some(Command::InstallService { config }) => service::install_service(config.as_deref()),
        #[cfg(feature = "broadcast")]
        Some(Command::Broadcast { group, message }) => {
            let groups = config
                .broadcast_groups
                .as_ref()
                .ok_or("no broadcast groups configured")?;
            let bg = groups
                .iter()
                .find(|g| g.name == group)
                .ok_or_else(|| format!("broadcast group '{}' not found", group))?;

            let tokens = broadcast::BroadcastTokens::from_config(&config);
            tracing::info!(
                group = %group,
                targets = bg.targets.len(),
                "Broadcasting to group"
            );
            let results = broadcast::broadcast(bg, &message, &tokens).await;
            broadcast::display_results(&results);

            let success = results.iter().filter(|r| r.success).count();
            let failed = results.iter().filter(|r| !r.success).count();
            tracing::info!(sent = success, failed = failed, "Broadcast complete");
            Ok(())
        }
        #[cfg(feature = "tui")]
        Some(Command::Tui) => {
            let model = agent::build_model(&config, cli.model_override.as_deref())?;
            let session_mgr = build_session_manager(&config);
            let sid = if let Some(ref id) = cli.session_id {
                id.clone()
            } else {
                session_mgr
                    .create_session()
                    .await
                    .map_err(|e| format!("failed to create session: {}", e))?
            };
            tui::run_tui(&config, model, &sid, cli.model_override.as_deref()).await
        }
        Some(Command::Memory {
            action,
            query,
            limit,
        }) => run_memory_command(&config, &action, query.as_deref(), limit).await,
        #[cfg(feature = "web")]
        Some(Command::Connect {
            url,
            session,
            token,
        }) => {
            let token = token.or_else(|| std::env::var("SYNAPSE_GATEWAY_TOKEN").ok());
            gateway::client::run_connect(&url, session.as_deref(), token.as_deref()).await
        }
        #[cfg(feature = "web")]
        Some(Command::Acp { transport, port }) => {
            let model = agent::build_model(&config, cli.model_override.as_deref())?;
            match transport.as_str() {
                "stdio" => acp::stdio::run_stdio(&config, model).await,
                "http" => {
                    // Mount ACP routes on the gateway server
                    tracing::info!(
                        port = port,
                        "ACP HTTP mode: use `synapse serve` with ACP routes enabled"
                    );
                    Ok(())
                }
                _ => {
                    Err(format!("unknown ACP transport: '{}'. Use: stdio, http", transport).into())
                }
            }
        }
        Some(Command::Models { action, name }) => {
            commands::run_models_command(&config, &action, name.as_deref())
        }
        Some(Command::Workflow {
            action,
            name,
            input,
            data,
        }) => {
            workflow::run_workflow_command(
                &config,
                &action,
                name.as_deref(),
                input.as_deref(),
                data.as_deref(),
            )
            .await
        }
        None => {
            // Backward compat: bare `synapse` or `synapse "message"` → chat mode
            run_chat(
                &config,
                cli.message.as_deref(),
                cli.session_id.as_deref(),
                cli.model_override.as_deref(),
            )
            .await
        }
    }
}

async fn run_chat(
    config: &SynapseConfig,
    message: Option<&str>,
    session_id: Option<&str>,
    model_override: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let model = agent::build_model(config, model_override)?;

    let session_mgr = build_session_manager(config);
    let memory = session_mgr.memory();

    // Long-term memory
    let sessions_dir = PathBuf::from(&config.base.paths.sessions_dir);
    let ltm = Arc::new(memory::LongTermMemory::new(
        sessions_dir.join("long_term_memory"),
        config.memory.clone(),
    ));
    ltm.load().await.ok();

    // Resolve or create session
    let sid = if let Some(id) = session_id {
        // Verify the session exists
        session_mgr
            .get_session(id)
            .await
            .map_err(|e| format!("failed to look up session '{}': {}", id, e))?
            .ok_or_else(|| format!("session '{}' not found", id))?;
        id.to_string()
    } else {
        session_mgr
            .create_session()
            .await
            .map_err(|e| format!("failed to create session: {}", e))?
    };

    let mut messages = memory.load(&sid).await.unwrap_or_default();

    // Prepend system prompt with project context
    let cwd = std::env::current_dir().unwrap_or_default();
    let mut system_prompt = config
        .base
        .agent
        .system_prompt
        .clone()
        .unwrap_or_else(|| "You are Synapse, a helpful AI assistant.".to_string());

    let workspace_dir = config.workspace_dir();
    let project_context = agent::load_project_context(&workspace_dir, &cwd, &config.context);
    if !project_context.is_empty() {
        system_prompt.push_str("\n\n# Project Context\n\n");
        system_prompt.push_str(&project_context);
    }

    if messages.is_empty() || !messages[0].is_system() {
        messages.insert(0, Message::system(&system_prompt));
    }

    // Create cost tracker
    let tracker = usage::create_tracker();

    if let Some(user_message) = message {
        repl::single_shot(model, &memory, &sid, &mut messages, user_message, &ltm).await
    } else {
        repl::repl(
            model,
            config,
            &session_mgr,
            &memory,
            &sid,
            &mut messages,
            tracker,
            ltm.clone(),
        )
        .await
    }
}

async fn run_memory_command(
    config: &SynapseConfig,
    action: &str,
    query: Option<&str>,
    limit: usize,
) -> Result<(), Box<dyn std::error::Error>> {
    let sessions_dir = PathBuf::from(&config.base.paths.sessions_dir);
    let ltm =
        memory::LongTermMemory::new(sessions_dir.join("long_term_memory"), config.memory.clone());
    ltm.load().await.ok();

    match action {
        "status" => {
            let count = ltm.count().await;
            let has_embeddings = ltm.uses_embeddings();
            let db_path = sessions_dir.join("long_term_memory").join("vectors.db");
            let db_size = std::fs::metadata(&db_path).map(|m| m.len()).unwrap_or(0);

            println!("{}", "--- Memory Status ---".bold());
            println!("  {} {}", "Entries:".bold(), count);
            println!(
                "  {} {}",
                "Embeddings:".bold(),
                if has_embeddings {
                    "active"
                } else {
                    "disabled (fake)"
                }
            );
            println!(
                "  {} {}",
                "Provider:".bold(),
                config.memory.embedding_provider
            );
            println!(
                "  {} {}",
                "Hybrid search:".bold(),
                if config.memory.ltm_hybrid_search {
                    "enabled"
                } else {
                    "disabled"
                }
            );
            println!(
                "  {} {} days",
                "Decay half-life:".bold(),
                config.memory.ltm_decay_half_life_days
            );
            println!(
                "  {} {}",
                "MMR lambda:".bold(),
                config.memory.ltm_mmr_lambda
            );
            if db_size > 0 {
                println!(
                    "  {} {:.1}KB",
                    "Vector DB size:".bold(),
                    db_size as f64 / 1024.0
                );
            }
            let store_dir = sessions_dir
                .join("long_term_memory")
                .join("synapse")
                .join("long_term_memory");
            if store_dir.exists() {
                let file_count = std::fs::read_dir(&store_dir)
                    .map(|d| d.count())
                    .unwrap_or(0);
                println!("  {} {} files", "Store files:".bold(), file_count);
            }
        }
        "list" => {
            let memories = ltm.list().await;
            if memories.is_empty() {
                println!("{}", "No long-term memories stored.".dimmed());
            } else {
                let shown = memories.len().min(limit);
                println!(
                    "{} ({} total, showing {})",
                    "Long-term Memories:".bold(),
                    memories.len(),
                    shown
                );
                for (i, (key, content)) in memories.iter().take(limit).enumerate() {
                    let preview = if content.len() > 100 {
                        format!("{}...", &content[..97])
                    } else {
                        content.clone()
                    };
                    let preview = preview.replace('\n', " ");
                    println!("  {}. [{}] {}", i + 1, key.dimmed(), preview);
                }
            }
        }
        "search" => {
            let q = query.ok_or("usage: synapse memory search <query>")?;
            let results = ltm.recall(q, limit).await;
            if results.is_empty() {
                println!("{}", "No relevant memories found.".dimmed());
            } else {
                println!(
                    "{} ({} results for '{}')",
                    "Search Results:".bold(),
                    results.len(),
                    q
                );
                for (i, content) in results.iter().enumerate() {
                    let preview = if content.len() > 200 {
                        format!("{}...", &content[..197])
                    } else {
                        content.clone()
                    };
                    let preview = preview.replace('\n', " ");
                    println!("  {}. {}", i + 1, preview);
                }
            }
        }
        "clear" => {
            let count = ltm.clear_all().await?;
            println!("{} Cleared {} memories", "memory:".green().bold(), count);
        }
        _ => {
            tracing::error!(action = %action, "Unknown memory action. Available: status, list, search, clear");
        }
    }
    Ok(())
}
