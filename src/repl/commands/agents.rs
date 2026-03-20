use colored::Colorize;

use super::CommandResult;
use crate::config::SynapseConfig;

pub fn cmd_skill(arg: &str) -> CommandResult {
    let parts: Vec<&str> = arg.splitn(2, ' ').collect();
    let action = parts.first().copied().unwrap_or("list");
    let skill_arg = parts.get(1).copied();

    match action {
        "list" | "ls" | "" => {
            if let Err(e) = crate::commands::run_skill_command("list", None) {
                eprintln!("{} {}", "error:".red().bold(), e);
            }
        }
        "info" => {
            if let Some(name) = skill_arg {
                if let Err(e) = crate::commands::run_skill_command("info", Some(name)) {
                    eprintln!("{} {}", "error:".red().bold(), e);
                }
            } else {
                eprintln!("{} Usage: /skill info <name>", "usage:".yellow().bold());
            }
        }
        _ => {
            eprintln!(
                "{} Unknown skill action '{}'. Use: list, info <name>",
                "warning:".yellow().bold(),
                action
            );
        }
    }
    CommandResult::Continue
}

pub fn cmd_subagents(arg: &str, config: &SynapseConfig) -> CommandResult {
    let parts: Vec<&str> = arg.splitn(2, ' ').collect();
    let action = parts.first().copied().unwrap_or("");

    match action {
        "" | "info" => {
            println!("{}", "--- Sub-Agent Configuration ---".bold());
            println!("  Enabled:        {}", config.subagent.enabled);
            println!("  Max depth:      {}", config.subagent.max_depth);
            println!("  Max concurrent: {}", config.subagent.max_concurrent);
            println!("  Timeout:        {}s", config.subagent.timeout_secs);
            println!();

            println!("{}", "--- Built-in Agents ---".bold());
            for name in &["Explore", "Plan", "Bash"] {
                if let Some(def) = synaptic::deep::builtin_agent_def(name) {
                    println!("  {} — {}", name.cyan(), def.description);
                }
            }
            println!();

            if !config.subagent.agents.is_empty() {
                println!("{}", "--- Config Agents ---".bold());
                for def in &config.subagent.agents {
                    println!("  {} — {}", def.name.cyan(), def.description);
                    if let Some(ref tp) = def.tool_profile {
                        println!("    profile: {}", tp);
                    }
                }
                println!();
            }

            let cwd = std::env::current_dir().unwrap_or_default();
            let discovered = crate::agent::discovery::discover_agents(&cwd);
            if !discovered.is_empty() {
                println!("{}", "--- Discovered Agents (.claude/agents/) ---".bold());
                for def in &discovered {
                    println!("  {} — {}", def.name.cyan(), def.description);
                }
                println!();
            }

            if !config.subagent.tool_profiles.is_empty() {
                println!("{}", "--- Tool Profiles ---".bold());
                for (name, tools) in &config.subagent.tool_profiles {
                    println!("  {}: {}", name.cyan(), tools.join(", "));
                }
            }
        }
        "help" => {
            println!("{}", "--- /subagents Commands ---".bold());
            println!("  {} — Show config + defined agents", "/subagents".cyan());
            println!("  {} — This help", "/subagents help".cyan());
        }
        _ => {
            eprintln!(
                "{} Unknown subagents action '{}'. Use: /subagents help",
                "warning:".yellow().bold(),
                action
            );
        }
    }
    CommandResult::Continue
}
