use crate::cli::SandboxAction;
use crate::config::SynapseConfig;

pub async fn handle_sandbox_cli(
    action: SandboxAction,
    config: &SynapseConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        SandboxAction::List { json } => {
            if let Some(ref sandbox_config) = config.sandbox {
                use super::orchestrator::SandboxOrchestrator;
                use super::registry::SandboxPersistentRegistry;
                use std::sync::Arc;
                use synaptic::deep::sandbox::SandboxProviderRegistry;

                let registry = Arc::new(SandboxProviderRegistry::new());
                let persistent =
                    SandboxPersistentRegistry::new(SandboxPersistentRegistry::default_path());
                let orch = SandboxOrchestrator::new(registry, sandbox_config.clone(), persistent);
                let instances = orch.list_all().await?;

                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&instances).unwrap_or_default()
                    );
                } else if instances.is_empty() {
                    eprintln!("No sandbox instances running.");
                } else {
                    eprintln!("Sandbox instances ({}):", instances.len());
                    for inst in &instances {
                        eprintln!(
                            "  {} scope={} provider={} image={}",
                            inst.runtime_id,
                            inst.scope_key,
                            inst.provider_id,
                            inst.image.as_deref().unwrap_or("-"),
                        );
                    }
                }
            } else {
                eprintln!("Sandbox not configured in synapse.toml");
            }
        }
        SandboxAction::Recreate {
            all,
            session,
            agent,
            force,
        } => {
            if let Some(ref sandbox_config) = config.sandbox {
                use super::orchestrator::{SandboxFilter, SandboxOrchestrator};
                use super::registry::SandboxPersistentRegistry;
                use std::sync::Arc;
                use synaptic::deep::sandbox::SandboxProviderRegistry;

                let filter = if all {
                    SandboxFilter::All
                } else if let Some(ref s) = session {
                    SandboxFilter::BySession(s.clone())
                } else if let Some(ref a) = agent {
                    SandboxFilter::ByAgent(a.clone())
                } else {
                    SandboxFilter::All
                };

                if !force {
                    eprintln!("This will destroy and recreate sandbox instances. Use --force to skip this prompt.");
                    eprint!("Continue? [y/N] ");
                    let mut input = String::new();
                    std::io::stdin().read_line(&mut input)?;
                    if !input.trim().eq_ignore_ascii_case("y") {
                        eprintln!("Aborted.");
                        return Ok(());
                    }
                }

                let registry = Arc::new(SandboxProviderRegistry::new());
                let persistent =
                    SandboxPersistentRegistry::new(SandboxPersistentRegistry::default_path());
                let orch = SandboxOrchestrator::new(registry, sandbox_config.clone(), persistent);
                let count = orch.recreate(&filter).await?;
                eprintln!("Recreated {} sandbox instance(s)", count);
            } else {
                eprintln!("Sandbox not configured in synapse.toml");
            }
        }
        SandboxAction::Explain {
            session,
            agent,
            json,
        } => {
            if let Some(ref sandbox_config) = config.sandbox {
                use super::orchestrator::SandboxOrchestrator;
                use super::registry::SandboxPersistentRegistry;
                use std::sync::Arc;
                use synaptic::deep::sandbox::SandboxProviderRegistry;

                let sess = session.as_deref().unwrap_or("main");
                let ag = agent.as_deref().unwrap_or("main");

                let registry = Arc::new(SandboxProviderRegistry::new());
                let persistent =
                    SandboxPersistentRegistry::new(SandboxPersistentRegistry::default_path());
                let orch = SandboxOrchestrator::new(registry, sandbox_config.clone(), persistent);
                let explanation = orch.explain(sess, ag);

                if json {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&explanation).unwrap_or_default()
                    );
                } else {
                    println!("{}", explanation);
                }
            } else {
                eprintln!("Sandbox not configured in synapse.toml");
            }
        }
    }
    Ok(())
}
