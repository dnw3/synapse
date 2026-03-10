//! Skill management CLI commands.
//!
//! Supports global skills (in `~/.claude/skills/`) and workspace-local
//! skills (in `.claude/skills/` relative to CWD).

use std::path::{Path, PathBuf};

use colored::Colorize;
use synaptic_deep::skill::load_manifest;

/// Run skill subcommand.
pub fn run_skill_command(
    action: &str,
    arg: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        "list" | "ls" => list_skills(),
        "info" => {
            let name = arg.ok_or("usage: synapse skill info <name>")?;
            skill_info(name)
        }
        "install" => {
            let source = arg.ok_or("usage: synapse skill install <path-or-hub-name>")?;
            // If source looks like a directory path, install locally; otherwise try ClawHub
            let source_path = std::path::Path::new(source);
            if source_path.exists() && source_path.is_dir() {
                install_skill(source)
            } else {
                // Try ClawHub installation
                let rt = tokio::runtime::Handle::current();
                rt.block_on(async {
                    let config = crate::config::SynapseConfig::load_or_default(None)?;
                    let hub = crate::hub::ClawHubClient::from_config(&config);
                    crate::hub::install::install_from_hub(&hub, source, None, false).await
                })
            }
        }
        "search" => {
            let query = arg.ok_or("usage: synapse skill search <query>")?;
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let config = crate::config::SynapseConfig::load_or_default(None)?;
                let hub = crate::hub::ClawHubClient::from_config(&config);
                let results = hub.search(query, 20).await?;
                if results.is_empty() {
                    println!("{}", "No skills found on ClawHub.".dimmed());
                } else {
                    println!(
                        "{:<25} {:<12} {:<15} DESCRIPTION",
                        "NAME", "VERSION", "AUTHOR"
                    );
                    println!("{}", "-".repeat(80));
                    for entry in &results {
                        println!(
                            "{:<25} {:<12} {:<15} {}",
                            entry.name, entry.version, entry.author, entry.description
                        );
                    }
                    println!("\n{} result(s). Install with: synapse skill install <name>", results.len());
                }
                Ok(())
            })
        }
        "update" => {
            let name = arg.ok_or("usage: synapse skill update <name>")?;
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let config = crate::config::SynapseConfig::load_or_default(None)?;
                let hub = crate::hub::ClawHubClient::from_config(&config);
                crate::hub::install::update_skill(&hub, name).await
            })
        }
        "enable" => {
            let name = arg.ok_or("usage: synapse skill enable <name>")?;
            enable_skill(name)
        }
        "disable" => {
            let name = arg.ok_or("usage: synapse skill disable <name>")?;
            disable_skill(name)
        }
        "remove" | "rm" => {
            let name = arg.ok_or("usage: synapse skill remove <name>")?;
            remove_skill(name)
        }
        _ => Err(format!(
            "unknown skill action: '{}'. Use: list, info, install, search, update, enable, disable, remove",
            action
        )
        .into()),
    }
}

/// Global skills directory: `~/.claude/skills/`
fn global_skills_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".claude")
        .join("skills")
}

/// Workspace-local skills directory: `.claude/skills/` relative to CWD.
fn workspace_skills_dir() -> Option<PathBuf> {
    let cwd = std::env::current_dir().ok()?;
    let dir = cwd.join(".claude").join("skills");
    if dir.exists() {
        Some(dir)
    } else {
        let legacy = cwd.join(".synapse").join("skills");
        if legacy.exists() {
            Some(legacy)
        } else {
            None
        }
    }
}

/// Check if a skill is disabled (has a `.disabled` marker file).
fn is_disabled(skill_dir: &Path) -> bool {
    skill_dir.join(".disabled").exists()
}

/// Scan a directory for skills — supports both SKILL.md (OpenClaw) and manifest.toml (legacy).
fn discover_all_skills(dir: &Path) -> Vec<(PathBuf, SkillInfo)> {
    let mut results = Vec::new();
    let entries = match std::fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return results,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let skill_md = path.join("SKILL.md");
        if skill_md.exists() {
            if let Ok(content) = std::fs::read_to_string(&skill_md) {
                if let Some(info) = parse_skill_md_info(&content) {
                    results.push((path, info));
                    continue;
                }
            }
        }
        let manifest_path = path.join("manifest.toml");
        if manifest_path.exists() {
            if let Ok(manifest) = load_manifest(&manifest_path) {
                results.push((
                    path,
                    SkillInfo {
                        name: manifest.name,
                        version: manifest.version,
                        description: manifest.description,
                    },
                ));
            }
        }
    }
    results
}

/// Minimal skill info for listing.
struct SkillInfo {
    name: String,
    version: String,
    description: String,
}

/// Parse SKILL.md frontmatter to extract name and description.
fn parse_skill_md_info(content: &str) -> Option<SkillInfo> {
    let content = content.trim_start_matches('\u{feff}');
    let mut lines = content.lines();
    if lines.next()?.trim() != "---" {
        return None;
    }
    let mut name = None;
    let mut description = None;
    for line in lines {
        let trimmed = line.trim();
        if trimmed == "---" {
            break;
        }
        if let Some(val) = trimmed.strip_prefix("name:") {
            name = Some(val.trim().trim_matches('"').trim_matches('\'').to_string());
        } else if let Some(val) = trimmed.strip_prefix("description:") {
            description = Some(val.trim().trim_matches('"').trim_matches('\'').to_string());
        }
    }
    Some(SkillInfo {
        name: name?,
        version: "-".to_string(),
        description: description.unwrap_or_default(),
    })
}

fn list_skills() -> Result<(), Box<dyn std::error::Error>> {
    let global_dir = global_skills_dir();
    let workspace_dir = workspace_skills_dir();

    let global_skills = if global_dir.exists() {
        discover_all_skills(&global_dir)
    } else {
        Vec::new()
    };

    let workspace_skills = workspace_dir
        .as_ref()
        .map(|d| discover_all_skills(d))
        .unwrap_or_default();

    if global_skills.is_empty() && workspace_skills.is_empty() {
        println!("{}", "No skills found.".dimmed());
        println!("  Global:    {}", global_dir.display());
        if let Some(ref ws) = workspace_dir {
            println!("  Workspace: {}", ws.display());
        } else {
            println!("  Workspace: .claude/skills/ (not found)");
        }
        println!(
            "\nInstall a skill: {}",
            "synapse skill install <path>".cyan()
        );
        return Ok(());
    }

    println!(
        "{:<20} {:<10} {:<10} {:<10} DESCRIPTION",
        "NAME", "VERSION", "SCOPE", "STATUS"
    );
    println!("{}", "-".repeat(75));

    for (path, info) in &global_skills {
        let status = if is_disabled(path) {
            "disabled".red().to_string()
        } else {
            "enabled".green().to_string()
        };
        println!(
            "{:<20} {:<10} {:<10} {:<10} {}",
            info.name, info.version, "global", status, info.description
        );
    }

    for (path, info) in &workspace_skills {
        let status = if is_disabled(path) {
            "disabled".red().to_string()
        } else {
            "enabled".green().to_string()
        };
        println!(
            "{:<20} {:<10} {:<10} {:<10} {}",
            info.name, info.version, "workspace", status, info.description
        );
    }

    let total = global_skills.len() + workspace_skills.len();
    println!("\n{} skill(s) found", total);
    Ok(())
}

fn skill_info(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let dirs_to_check: Vec<(&str, PathBuf)> = {
        let mut v = Vec::new();
        if let Some(ws) = workspace_skills_dir() {
            v.push(("workspace", ws));
        }
        v.push(("global", global_skills_dir()));
        v
    };

    for (scope, dir) in &dirs_to_check {
        let skill_dir = dir.join(name);

        let skill_md = skill_dir.join("SKILL.md");
        if skill_md.exists() {
            if let Ok(content) = std::fs::read_to_string(&skill_md) {
                if let Some(info) = parse_skill_md_info(&content) {
                    println!("{} {}", "Name:".bold(), info.name);
                    println!("{} {}", "Description:".bold(), info.description);
                    println!("{} {}", "Format:".bold(), "SKILL.md (OpenClaw)");
                    println!("{} {}", "Scope:".bold(), scope);
                    println!(
                        "{} {}",
                        "Status:".bold(),
                        if is_disabled(&skill_dir) {
                            "disabled".red()
                        } else {
                            "enabled".green()
                        }
                    );
                    println!("{} {}", "Path:".bold(), skill_dir.display());
                    return Ok(());
                }
            }
        }

        let manifest_path = skill_dir.join("manifest.toml");
        if manifest_path.exists() {
            let manifest =
                load_manifest(&manifest_path).map_err(|e| format!("failed to load manifest: {}", e))?;

            println!("{} {}", "Name:".bold(), manifest.name);
            println!("{} {}", "Version:".bold(), manifest.version);
            println!("{} {}", "Description:".bold(), manifest.description);
            println!("{} {}", "Format:".bold(), "manifest.toml (legacy)");
            println!("{} {}", "Scope:".bold(), scope);
            println!(
                "{} {}",
                "Status:".bold(),
                if is_disabled(&skill_dir) {
                    "disabled".red()
                } else {
                    "enabled".green()
                }
            );
            println!("{} {}", "Path:".bold(), skill_dir.display());
            if let Some(ref author) = manifest.author {
                println!("{} {}", "Author:".bold(), author);
            }
            if !manifest.tags.is_empty() {
                println!("{} {}", "Tags:".bold(), manifest.tags.join(", "));
            }
            return Ok(());
        }
    }

    Err(format!("skill '{}' not found in any skills directory", name).into())
}

/// Install a skill from a source path (directory containing SKILL.md or manifest.toml).
fn install_skill(source: &str) -> Result<(), Box<dyn std::error::Error>> {
    let source_path = Path::new(source);
    if !source_path.exists() {
        return Err(format!("source path '{}' does not exist", source).into());
    }
    if !source_path.is_dir() {
        return Err("source must be a directory containing SKILL.md or manifest.toml".into());
    }

    let skill_md = source_path.join("SKILL.md");
    let manifest_path = source_path.join("manifest.toml");

    let (skill_name, version) = if skill_md.exists() {
        let content = std::fs::read_to_string(&skill_md)?;
        let info = parse_skill_md_info(&content)
            .ok_or("SKILL.md has invalid or missing frontmatter (requires 'name' field)")?;
        (info.name, info.version)
    } else if manifest_path.exists() {
        let manifest =
            load_manifest(&manifest_path).map_err(|e| format!("invalid manifest: {}", e))?;
        (manifest.name, manifest.version)
    } else {
        return Err(format!(
            "no SKILL.md or manifest.toml found in '{}'",
            source_path.display()
        )
        .into());
    };

    let dest_dir = global_skills_dir();
    std::fs::create_dir_all(&dest_dir)?;

    let target = dest_dir.join(&skill_name);
    if target.exists() {
        return Err(format!(
            "skill '{}' already installed at {}. Remove it first.",
            skill_name,
            target.display()
        )
        .into());
    }

    super::copy_dir_recursive(source_path, &target)?;

    println!(
        "{} Installed skill '{}' v{} to {}",
        "install:".green().bold(),
        skill_name,
        version,
        target.display()
    );
    Ok(())
}

/// Enable a previously disabled skill.
fn enable_skill(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let skill_dir = find_skill_dir(name)?;
    let marker = skill_dir.join(".disabled");

    if !marker.exists() {
        println!("Skill '{}' is already enabled.", name);
        return Ok(());
    }

    std::fs::remove_file(&marker)?;
    println!(
        "{} Skill '{}' enabled",
        "enable:".green().bold(),
        name
    );
    Ok(())
}

/// Disable a skill by creating a `.disabled` marker.
fn disable_skill(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let skill_dir = find_skill_dir(name)?;
    let marker = skill_dir.join(".disabled");

    if marker.exists() {
        println!("Skill '{}' is already disabled.", name);
        return Ok(());
    }

    std::fs::write(&marker, "")?;
    println!(
        "{} Skill '{}' disabled",
        "disable:".yellow().bold(),
        name
    );
    Ok(())
}

/// Remove a skill entirely.
fn remove_skill(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let skill_dir = find_skill_dir(name)?;
    std::fs::remove_dir_all(&skill_dir)?;
    println!(
        "{} Skill '{}' removed from {}",
        "remove:".red().bold(),
        name,
        skill_dir.display()
    );
    Ok(())
}

/// Find the directory for a named skill (checks workspace first, then global).
fn find_skill_dir(name: &str) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if let Some(ws) = workspace_skills_dir() {
        let dir = ws.join(name);
        if dir.exists() {
            return Ok(dir);
        }
    }

    let dir = global_skills_dir().join(name);
    if dir.exists() {
        return Ok(dir);
    }

    Err(format!("skill '{}' not found", name).into())
}
