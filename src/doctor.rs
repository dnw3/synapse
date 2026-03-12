use colored::Colorize;
use synaptic::core::{ChatModel, ChatRequest, Message};

use crate::agent;
use crate::config::SynapseConfig;

struct CheckResult {
    name: String,
    passed: bool,
    detail: String,
}

impl CheckResult {
    fn pass(name: &str, detail: &str) -> Self {
        Self {
            name: name.to_string(),
            passed: true,
            detail: detail.to_string(),
        }
    }

    fn fail(name: &str, detail: &str) -> Self {
        Self {
            name: name.to_string(),
            passed: false,
            detail: detail.to_string(),
        }
    }
}

/// Run all doctor checks and print results.
pub async fn run_doctor(config: &SynapseConfig) -> Result<(), Box<dyn std::error::Error>> {
    println!();
    println!("{}", "═══ Synapse Doctor ═══".bold().cyan());
    println!();

    let mut results = Vec::new();

    // 1. Config file
    results.push(check_config());

    // 2. API key
    results.push(check_api_key(config));

    // 3. Model connectivity
    results.push(check_model(config).await);

    // 4. Fallback models
    if let Some(ref fallbacks) = config.fallback_models {
        for name in fallbacks {
            results.push(check_fallback_model(config, name).await);
        }
    }

    // 5. MCP servers
    if let Some(ref mcps) = config.base.mcp {
        for mc in mcps {
            results.push(check_mcp_server(mc));
        }
    }

    // 6. Docker
    results.push(check_docker(config).await);

    // 7. Sessions directory
    results.push(check_sessions_dir(config));

    // Print results
    println!("{}", "─── Results ───".bold());
    let mut passed = 0;
    let mut failed = 0;
    let mut skipped = 0;

    for r in &results {
        if r.detail == "skipped" {
            skipped += 1;
            println!("  {} {} — {}", "◦".dimmed(), r.name, r.detail.dimmed());
        } else if r.passed {
            passed += 1;
            println!("  {} {} — {}", "✓".green(), r.name, r.detail.green());
        } else {
            failed += 1;
            println!("  {} {} — {}", "✗".red(), r.name, r.detail.red());
        }
    }

    println!();
    if failed == 0 {
        println!(
            "{} All checks passed ({} passed, {} skipped)",
            "✓".green().bold(),
            passed,
            skipped
        );
    } else {
        println!(
            "{} {} passed, {} failed, {} skipped",
            "✗".red().bold(),
            passed,
            failed,
            skipped
        );
    }
    println!();

    Ok(())
}

fn check_config() -> CheckResult {
    let paths = [
        "synapse.toml",
        "synaptic.json",
        "synaptic.yaml",
        "synaptic.yml",
    ];

    for p in &paths {
        if std::path::Path::new(p).exists() {
            return CheckResult::pass("Config file", &format!("found {}", p));
        }
    }

    // Check ~/.synaptic/config.toml
    if let Some(home) = dirs::home_dir() {
        let home_config = home.join(".synaptic").join("config.toml");
        if home_config.exists() {
            return CheckResult::pass("Config file", &format!("found {}", home_config.display()));
        }
    }

    CheckResult::fail(
        "Config file",
        "not found — run `synapse init` to create one",
    )
}

fn check_api_key(config: &SynapseConfig) -> CheckResult {
    let env_var = &config.base.model.api_key_env;
    match std::env::var(env_var) {
        Ok(val) if !val.is_empty() => {
            let masked = format!(
                "{}...{}",
                &val[..4.min(val.len())],
                &val[val.len().saturating_sub(4)..]
            );
            CheckResult::pass("API key", &format!("{} = {}", env_var, masked))
        }
        _ => CheckResult::fail("API key", &format!("{} not set", env_var)),
    }
}

async fn check_model(config: &SynapseConfig) -> CheckResult {
    let model = match agent::build_model(config, None) {
        Ok(m) => m,
        Err(e) => return CheckResult::fail("Model", &format!("build error: {}", e)),
    };

    match test_model_connectivity(&*model, &config.base.model.model).await {
        Ok(()) => CheckResult::pass("Model", &format!("{} reachable", config.base.model.model)),
        Err(e) => CheckResult::fail("Model", &format!("{} — {}", config.base.model.model, e)),
    }
}

async fn check_fallback_model(config: &SynapseConfig, name: &str) -> CheckResult {
    let model = match agent::build_model_by_name(config, name) {
        Ok(m) => m,
        Err(e) => {
            return CheckResult::fail(
                &format!("Fallback ({})", name),
                &format!("build error: {}", e),
            )
        }
    };

    match test_model_connectivity(&*model, name).await {
        Ok(()) => CheckResult::pass(&format!("Fallback ({})", name), "reachable"),
        Err(e) => CheckResult::fail(&format!("Fallback ({})", name), &e),
    }
}

async fn test_model_connectivity(model: &dyn ChatModel, _name: &str) -> Result<(), String> {
    let request = ChatRequest::new(vec![Message::human("hi")]);
    match tokio::time::timeout(std::time::Duration::from_secs(15), model.chat(request)).await {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(e)) => Err(format!("{}", e)),
        Err(_) => Err("timeout (15s)".to_string()),
    }
}

fn check_mcp_server(mc: &synaptic::config::McpServerConfig) -> CheckResult {
    match mc.transport.as_str() {
        "stdio" => {
            if let Some(ref cmd) = mc.command {
                // Check if command exists in PATH
                match which::which(cmd) {
                    Ok(_) => CheckResult::pass(
                        &format!("MCP ({})", mc.name),
                        &format!("command '{}' found", cmd),
                    ),
                    Err(_) => CheckResult::fail(
                        &format!("MCP ({})", mc.name),
                        &format!("command '{}' not found in PATH", cmd),
                    ),
                }
            } else {
                CheckResult::fail(&format!("MCP ({})", mc.name), "no command specified")
            }
        }
        "sse" | "http" => {
            if mc.url.is_some() {
                CheckResult::pass(
                    &format!("MCP ({})", mc.name),
                    &format!("{} endpoint configured", mc.transport),
                )
            } else {
                CheckResult::fail(&format!("MCP ({})", mc.name), "no URL specified")
            }
        }
        _ => CheckResult::fail(
            &format!("MCP ({})", mc.name),
            &format!("unknown transport '{}'", mc.transport),
        ),
    }
}

async fn check_docker(config: &SynapseConfig) -> CheckResult {
    let docker_enabled = config.docker.as_ref().map(|d| d.enabled).unwrap_or(false);

    if !docker_enabled {
        return CheckResult {
            name: "Docker".to_string(),
            passed: true,
            detail: "skipped".to_string(),
        };
    }

    match tokio::process::Command::new("docker")
        .arg("info")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .await
    {
        Ok(status) if status.success() => CheckResult::pass("Docker", "available"),
        Ok(_) => CheckResult::fail("Docker", "not running"),
        Err(_) => CheckResult::fail("Docker", "not installed"),
    }
}

fn check_sessions_dir(config: &SynapseConfig) -> CheckResult {
    let dir = &config.base.paths.sessions_dir;
    let path = std::path::Path::new(dir);
    if path.exists() {
        CheckResult::pass("Sessions dir", &format!("{} exists", dir))
    } else {
        // Will be auto-created, so this is just informational
        CheckResult::pass("Sessions dir", &format!("{} (will be created)", dir))
    }
}
