use colored::Colorize;

use crate::agent::registry::ModelRegistry;
use crate::config::SynapseConfig;

pub fn run_models_command(
    config: &SynapseConfig,
    action: &str,
    _name: Option<&str>,
) -> crate::error::Result<()> {
    let registry = ModelRegistry::from_config(config);

    match action {
        "list" | "ls" => {
            let entries = registry.list();
            if entries.is_empty() {
                println!(
                    "{} No models in catalog. Add [[models]] to your config.",
                    "model:".dimmed()
                );
                println!();
                println!(
                    "Current default: {} ({})",
                    config.model_config().model.cyan(),
                    config.model_config().provider.dimmed()
                );
            } else {
                println!(
                    "{:30} {:15} {}",
                    "NAME".bold(),
                    "PROVIDER".bold(),
                    "ALIASES".bold()
                );
                println!("{}", "-".repeat(65));
                for entry in &entries {
                    let aliases = if entry.aliases.is_empty() {
                        "-".to_string()
                    } else {
                        entry.aliases.join(", ")
                    };
                    let provider = entry.provider.as_deref().unwrap_or("-");
                    println!(
                        "{:30} {:15} {}",
                        entry.name.cyan(),
                        provider.dimmed(),
                        aliases
                    );
                }
                println!();
                println!(
                    "Default: {} (from [model] config)",
                    config.model_config().model.cyan()
                );
            }
        }

        "status" => {
            println!("{}", "--- Model Status ---".bold());
            println!(
                "  {} {}",
                "Default model:".bold(),
                config.model_config().model.cyan()
            );
            println!(
                "  {} {}",
                "Provider:".bold(),
                config.model_config().provider.dimmed()
            );

            if let Some(ref url) = config.model_config().base_url {
                println!("  {} {}", "Base URL:".bold(), url.dimmed());
            }
            if let Some(temp) = config.model_config().temperature {
                println!("  {} {}", "Temperature:".bold(), temp);
            }
            if let Some(max) = config.model_config().max_tokens {
                println!("  {} {}", "Max tokens:".bold(), max);
            }

            let api_key_set = config.resolve_api_key().is_ok();
            println!(
                "  {} {}",
                "API key:".bold(),
                if api_key_set {
                    "set".green()
                } else {
                    "missing".red()
                }
            );

            if let Some(ref fallbacks) = config.fallback_models {
                println!(
                    "  {} {}",
                    "Fallbacks:".bold(),
                    fallbacks.join(", ").dimmed()
                );
            }

            let catalog_count = registry.list().len();
            let alias_count = registry.aliases().len();
            println!(
                "  {} {} models, {} aliases",
                "Catalog:".bold(),
                catalog_count,
                alias_count
            );

            // Show provider key status
            if let Some(ref providers) = config.provider_catalog {
                if !providers.is_empty() {
                    println!();
                    println!("{}", "--- Provider Key Status ---".bold());
                    for prov in providers {
                        let key_status = if let Some(ref env) = prov.api_keys_env {
                            match std::env::var(env) {
                                Ok(val) => {
                                    let count =
                                        val.split(',').filter(|s| !s.trim().is_empty()).count();
                                    format!("{} keys (rotation)", count).green().to_string()
                                }
                                Err(_) => format!("{} not set", env).red().to_string(),
                            }
                        } else if let Some(ref env) = prov.api_key_env {
                            if std::env::var(env).is_ok() {
                                "set".green().to_string()
                            } else {
                                format!("{} not set", env).red().to_string()
                            }
                        } else {
                            "using default key".dimmed().to_string()
                        };
                        println!(
                            "  {} [{}]: {}",
                            prov.name.cyan(),
                            prov.base_url.dimmed(),
                            key_status
                        );
                    }
                }
            }
        }

        "aliases" => {
            let aliases = registry.aliases();
            if aliases.is_empty() {
                println!(
                    "{}",
                    "No aliases defined. Add `aliases` to [[models]] entries.".dimmed()
                );
            } else {
                println!("{:20} {}", "ALIAS".bold(), "MODEL".bold());
                println!("{}", "-".repeat(50));
                for (alias, canonical) in &aliases {
                    println!("{:20} {}", alias.cyan(), canonical);
                }
            }
        }

        _ => {
            eprintln!(
                "{} unknown action '{}'. Available: list, status, aliases",
                "error:".red().bold(),
                action
            );
        }
    }

    Ok(())
}
