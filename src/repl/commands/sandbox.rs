use std::sync::Arc;

use colored::Colorize;

use super::CommandResult;
use crate::sandbox::orchestrator::{SandboxFilter, SandboxOrchestrator};

pub async fn cmd_sandbox(
    arg: &str,
    orchestrator: &Arc<SandboxOrchestrator>,
    session_id: &str,
) -> CommandResult {
    let (sub, sub_arg) = match arg.split_once(' ') {
        Some((s, a)) => (s.trim(), a.trim()),
        None => (arg.trim(), ""),
    };

    match sub {
        "list" | "ls" => cmd_list(orchestrator).await,
        "recreate" => cmd_recreate(orchestrator).await,
        "explain" => cmd_explain(orchestrator, session_id, sub_arg),
        "status" => cmd_status(orchestrator, session_id),
        "" => {
            eprintln!(
                "{} Usage: /sandbox <list|recreate|explain|status>",
                "sandbox:".yellow().bold()
            );
            CommandResult::Continue
        }
        other => {
            eprintln!(
                "{} Unknown subcommand '{}'. Use: list, recreate, explain, status",
                "sandbox:".yellow().bold(),
                other
            );
            CommandResult::Continue
        }
    }
}

async fn cmd_list(orchestrator: &Arc<SandboxOrchestrator>) -> CommandResult {
    match orchestrator.list_all().await {
        Ok(instances) => {
            if instances.is_empty() {
                println!("{}", "No sandbox instances running.".dimmed());
            } else {
                println!(
                    "{} ({} total)",
                    "Sandbox Instances:".bold(),
                    instances.len()
                );
                println!(
                    "  {:<16} {:<12} {:<20} {:<20}",
                    "RUNTIME ID".dimmed(),
                    "PROVIDER".dimmed(),
                    "SCOPE".dimmed(),
                    "CREATED".dimmed(),
                );
                for info in &instances {
                    let id_short = if info.runtime_id.len() > 14 {
                        format!("{}…", &info.runtime_id[..13])
                    } else {
                        info.runtime_id.clone()
                    };
                    println!(
                        "  {:<16} {:<12} {:<20} {:<20}",
                        id_short,
                        info.provider_id,
                        info.scope_key,
                        info.created_at.format("%Y-%m-%d %H:%M"),
                    );
                }
            }
        }
        Err(e) => eprintln!("{} {}", "error:".red().bold(), e),
    }
    CommandResult::Continue
}

async fn cmd_recreate(orchestrator: &Arc<SandboxOrchestrator>) -> CommandResult {
    eprintln!(
        "{} Destroying all sandbox instances...",
        "sandbox:".yellow().bold()
    );
    match orchestrator.recreate(&SandboxFilter::All).await {
        Ok(count) => {
            eprintln!(
                "{} Destroyed {} instance(s). They will be recreated on next use.",
                "sandbox:".green().bold(),
                count
            );
        }
        Err(e) => eprintln!("{} {}", "error:".red().bold(), e),
    }
    CommandResult::Continue
}

fn cmd_explain(
    orchestrator: &Arc<SandboxOrchestrator>,
    session_id: &str,
    agent_id: &str,
) -> CommandResult {
    let agent = if agent_id.is_empty() {
        "main"
    } else {
        agent_id
    };
    let explanation = orchestrator.explain(session_id, agent);
    println!("{}", "--- Sandbox Explanation ---".bold());
    println!("{explanation}");
    CommandResult::Continue
}

fn cmd_status(orchestrator: &Arc<SandboxOrchestrator>, session_id: &str) -> CommandResult {
    let explanation = orchestrator.explain(session_id, "main");
    println!("{}", "--- Sandbox Status ---".bold());
    println!("  {} {}", "Session:".bold(), explanation.session_key.cyan());
    println!(
        "  {} {}",
        "Runtime:".bold(),
        if explanation.is_sandboxed {
            "SANDBOXED".green()
        } else {
            "HOST".yellow()
        }
    );
    println!("  {} {}", "Mode:".bold(), explanation.mode.dimmed());
    println!("  {} {}", "Backend:".bold(), explanation.backend.dimmed());
    CommandResult::Continue
}
