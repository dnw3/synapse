use std::sync::Arc;

use colored::Colorize;
use synaptic::callbacks::CostTrackingCallback;
use synaptic::core::{ChatModel, Message, ThinkingConfig};

use super::CommandResult;
use crate::agent;
use crate::config::SynapseConfig;

pub async fn cmd_model(
    arg: &str,
    config: &SynapseConfig,
    model: &mut Arc<dyn ChatModel>,
    tracker: &Arc<CostTrackingCallback>,
    current_model_name: &mut String,
) -> CommandResult {
    let sub = arg.split_whitespace().next().unwrap_or("");
    match sub {
        "" => {
            println!("{} {}", "Current model:".bold(), current_model_name.cyan());
        }
        "list" | "ls" => {
            let registry = agent::registry::ModelRegistry::from_config(config);
            let entries = registry.list();
            if entries.is_empty() {
                println!(
                    "{} No models in catalog. Add [[models]] to config.",
                    "model:".dimmed()
                );
            } else {
                println!("{}", "--- Model Catalog ---".bold());
                for entry in &entries {
                    let is_current = entry.name == *current_model_name
                        || entry
                            .aliases
                            .iter()
                            .any(|a| a == current_model_name.as_str());
                    let marker = if is_current { " *" } else { "" };
                    let aliases = if entry.aliases.is_empty() {
                        String::new()
                    } else {
                        format!(" ({})", entry.aliases.join(", "))
                    };
                    let provider = entry.provider.as_deref().unwrap_or("-");
                    println!(
                        "  {} [{}]{}{}",
                        entry.name.cyan(),
                        provider.dimmed(),
                        aliases.dimmed(),
                        marker.green()
                    );
                }
            }
        }
        "aliases" => {
            let registry = agent::registry::ModelRegistry::from_config(config);
            let aliases = registry.aliases();
            if aliases.is_empty() {
                println!("{}", "No aliases defined.".dimmed());
            } else {
                println!("{}", "--- Model Aliases ---".bold());
                for (alias, canonical) in &aliases {
                    println!("  {} -> {}", alias.cyan(), canonical);
                }
            }
        }
        "status" => {
            let registry = agent::registry::ModelRegistry::from_config(config);
            println!("{}", "--- Model Status ---".bold());
            println!("  {} {}", "Current:".bold(), current_model_name.cyan());
            println!(
                "  {} {}",
                "Provider:".bold(),
                config.base.model.provider.dimmed()
            );
            if let Some(temp) = config.base.model.temperature {
                println!("  {} {}", "Temperature:".bold(), temp);
            }
            if let Some(max) = config.base.model.max_tokens {
                println!("  {} {}", "Max tokens:".bold(), max);
            }
            if let Some(ref fallbacks) = config.fallback_models {
                println!(
                    "  {} {}",
                    "Fallbacks:".bold(),
                    fallbacks.join(", ").dimmed()
                );
            }
            // Show registry provider info if model is from catalog
            if let Some(prov) = registry.provider_for(current_model_name) {
                println!("  {} {}", "Base URL:".bold(), prov.base_url.dimmed());
                let key_status = if prov.api_keys_env.is_some() {
                    "multi-key rotation"
                } else if prov.api_key_env.is_some() {
                    "single key"
                } else {
                    "default"
                };
                println!("  {} {}", "Key mode:".bold(), key_status);
            }
            println!("  {} {}", "Catalog size:".bold(), registry.list().len());
        }
        _ => {
            // Switch model (name or alias)
            match agent::build_model_by_name(config, arg) {
                Ok(new_model) => {
                    // Resolve to canonical name if it's an alias
                    let registry = agent::registry::ModelRegistry::from_config(config);
                    let display_name = registry.canonical_name(arg).unwrap_or(arg);
                    *model = new_model;
                    *current_model_name = display_name.to_string();
                    tracker.set_model(display_name).await;
                    if display_name != arg {
                        eprintln!(
                            "{} Switched to {} (alias: {})",
                            "model:".green().bold(),
                            display_name.cyan(),
                            arg.dimmed()
                        );
                    } else {
                        eprintln!("{} Switched to {}", "model:".green().bold(), arg.cyan());
                    }
                }
                Err(e) => {
                    eprintln!("{} Failed to switch model: {}", "error:".red().bold(), e);
                }
            }
        }
    }
    CommandResult::Continue
}

pub fn cmd_verbose(verbose: &mut bool) -> CommandResult {
    *verbose = !*verbose;
    eprintln!(
        "{} Verbose mode {}",
        "verbose:".green().bold(),
        if *verbose { "enabled" } else { "disabled" }
    );
    CommandResult::Continue
}

pub fn cmd_think(
    arg: &str,
    thinking: &mut Option<ThinkingConfig>,
    messages: &mut Vec<Message>,
) -> CommandResult {
    let level = if arg.is_empty() { "medium" } else { arg };
    match level {
        "off" | "none" => {
            *thinking = None;
            messages.retain(|m| !(m.is_system() && m.content().starts_with("[Thinking mode:")));
            eprintln!("{} Thinking mode disabled", "think:".green().bold());
        }
        "low" | "minimal" => {
            *thinking = Some(ThinkingConfig {
                enabled: true,
                budget_tokens: Some(2000),
            });
            eprintln!(
                "{} Thinking level set to '{}' (budget: 2000 tokens)",
                "think:".green().bold(),
                level
            );
        }
        "medium" => {
            *thinking = Some(ThinkingConfig {
                enabled: true,
                budget_tokens: Some(10000),
            });
            eprintln!(
                "{} Thinking level set to '{}' (budget: 10000 tokens)",
                "think:".green().bold(),
                level
            );
        }
        "high" => {
            *thinking = Some(ThinkingConfig {
                enabled: true,
                budget_tokens: Some(50000),
            });
            eprintln!(
                "{} Thinking level set to '{}' (budget: 50000 tokens)",
                "think:".green().bold(),
                level
            );
        }
        _ => {
            if let Ok(budget) = level.parse::<u32>() {
                *thinking = Some(ThinkingConfig {
                    enabled: true,
                    budget_tokens: Some(budget),
                });
                eprintln!(
                    "{} Thinking enabled with custom budget: {} tokens",
                    "think:".green().bold(),
                    budget
                );
            } else {
                eprintln!(
                    "{} Unknown level '{}'. Use: off, low, medium, high, or a number (token budget)",
                    "warning:".yellow().bold(),
                    level
                );
            }
        }
    }
    CommandResult::Continue
}
