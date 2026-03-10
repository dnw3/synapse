//! Skill installation logic for ClawHub skills.

use std::path::{Path, PathBuf};

use colored::Colorize;
use serde::{Deserialize, Serialize};

/// Manifest written to installed skill directory for tracking.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledManifest {
    pub name: String,
    pub version: String,
    pub source: String,
    pub installed_at: String,
}

/// Install a skill from ClawHub into the global skills directory.
pub async fn install_from_hub(
    hub: &super::ClawHubClient,
    name: &str,
    version: Option<&str>,
    upgrade: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let global_dir = global_skills_dir();
    std::fs::create_dir_all(&global_dir)?;

    let target = global_dir.join(name);
    if target.exists() && !upgrade {
        return Err(format!(
            "skill '{}' already installed at {}. Use --upgrade to overwrite.",
            name,
            target.display()
        )
        .into());
    }

    tracing::info!(package = %name, "installing from hub");

    // Get skill detail for metadata
    let detail = hub.get(name, version).await?;

    // Download SKILL.md
    let skill_md = hub.download_skill_md(name, version).await?;

    // Create/overwrite target directory
    if target.exists() {
        std::fs::remove_dir_all(&target)?;
    }
    std::fs::create_dir_all(&target)?;

    // Write SKILL.md
    std::fs::write(target.join("SKILL.md"), &skill_md)?;

    // Write install manifest
    let manifest = InstalledManifest {
        name: detail.name.clone(),
        version: detail.version.clone(),
        source: format!("clawhub:{}", name),
        installed_at: chrono_now(),
    };
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(target.join("manifest.json"), manifest_json)?;

    tracing::info!(
        name = %detail.name,
        version = %detail.version,
        path = %target.display(),
        "installed skill from hub"
    );
    println!(
        "{} Installed skill '{}' v{} from ClawHub to {}",
        "install:".green().bold(),
        detail.name,
        detail.version,
        target.display()
    );

    // Check requirements if the SKILL.md specifies them
    check_requirements(&skill_md);

    Ok(())
}

/// Check for required bins/env in SKILL.md frontmatter and warn if missing.
fn check_requirements(skill_md: &str) {
    let mut in_frontmatter = false;
    for line in skill_md.lines() {
        let trimmed = line.trim();
        if trimmed == "---" {
            if in_frontmatter {
                break;
            }
            in_frontmatter = true;
            continue;
        }
        if !in_frontmatter {
            continue;
        }

        if let Some(bins) = trimmed.strip_prefix("required-bins:") {
            let bins: Vec<&str> = bins.split(',').map(|s| s.trim().trim_matches('"')).collect();
            for bin in bins {
                if !bin.is_empty() && which::which(bin).is_err() {
                    tracing::warn!(binary = %bin, "required binary not found in PATH");
                }
            }
        }
        if let Some(envs) = trimmed.strip_prefix("required-env:") {
            let envs: Vec<&str> = envs.split(',').map(|s| s.trim().trim_matches('"')).collect();
            for env_var in envs {
                if !env_var.is_empty() && std::env::var(env_var).is_err() {
                    tracing::warn!(env_var = %env_var, "required env var not set");
                }
            }
        }
    }
}

/// Update an installed hub skill to the latest version.
pub async fn update_skill(
    hub: &super::ClawHubClient,
    name: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let target = global_skills_dir().join(name);
    if !target.exists() {
        return Err(format!("skill '{}' not installed", name).into());
    }

    // Check if it was installed from hub
    let manifest_path = target.join("manifest.json");
    if manifest_path.exists() {
        let content = std::fs::read_to_string(&manifest_path)?;
        let manifest: InstalledManifest = serde_json::from_str(&content)?;
        if !manifest.source.starts_with("clawhub:") {
            return Err(format!("skill '{}' was not installed from ClawHub", name).into());
        }
    }

    install_from_hub(hub, name, None, true).await
}

fn global_skills_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".claude")
        .join("skills")
}

fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", dur.as_secs())
}
