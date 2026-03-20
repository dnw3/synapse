//! Skill management CLI commands.
//!
//! Supports global skills (in `~/.claude/skills/`) and workspace-local
//! skills (in `.claude/skills/` relative to CWD).

use std::path::{Path, PathBuf};

use colored::Colorize;
use synaptic::deep::skill::load_manifest;

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
                        "{:<30} {:<12} SUMMARY",
                        "SLUG", "VERSION"
                    );
                    println!("{}", "-".repeat(80));
                    for entry in &results {
                        println!(
                            "{:<30} {:<12} {}",
                            entry.slug,
                            entry.version.as_deref().unwrap_or("-"),
                            entry.summary.as_deref().unwrap_or("")
                        );
                    }
                    println!("\n{} result(s). Install with: synapse skill install <slug>", results.len());
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
        "publish" => {
            let path = arg.ok_or("usage: synapse skill publish <path> [--version VER]")?;
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let config = crate::config::SynapseConfig::load_or_default(None)?;
                let hub = crate::hub::ClawHubClient::from_config(&config);
                publish_skill(&hub, path).await
            })
        }
        "star" => {
            let name = arg.ok_or("usage: synapse skill star <name>")?;
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let config = crate::config::SynapseConfig::load_or_default(None)?;
                let hub = crate::hub::ClawHubClient::from_config(&config);
                hub.star(name).await?;
                println!("{} Starred '{}'", "star:".yellow().bold(), name);
                Ok(())
            })
        }
        "unstar" => {
            let name = arg.ok_or("usage: synapse skill unstar <name>")?;
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let config = crate::config::SynapseConfig::load_or_default(None)?;
                let hub = crate::hub::ClawHubClient::from_config(&config);
                hub.unstar(name).await?;
                println!("{} Unstarred '{}'", "unstar:".dimmed(), name);
                Ok(())
            })
        }
        "versions" => {
            let name = arg.ok_or("usage: synapse skill versions <name>")?;
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let config = crate::config::SynapseConfig::load_or_default(None)?;
                let hub = crate::hub::ClawHubClient::from_config(&config);
                let versions = hub.list_versions(name).await?;
                if versions.is_empty() {
                    println!("{}", "No versions found.".dimmed());
                } else {
                    println!("{:<15} {:<25} DOWNLOADS", "VERSION", "CREATED");
                    println!("{}", "-".repeat(50));
                    for v in &versions {
                        println!(
                            "{:<15} {:<25} {}",
                            v.version,
                            v.created_at.as_deref().unwrap_or("-"),
                            v.downloads.unwrap_or(0)
                        );
                    }
                }
                Ok(())
            })
        }
        "doctor" => doctor_skills(),
        "whoami" => {
            let rt = tokio::runtime::Handle::current();
            rt.block_on(async {
                let config = crate::config::SynapseConfig::load_or_default(None)?;
                let hub = crate::hub::ClawHubClient::from_config(&config);
                let user = hub.whoami().await?;
                println!(
                    "{} {}",
                    "Handle:".bold(),
                    user.handle.as_deref().unwrap_or("(not set)")
                );
                println!(
                    "{} {}",
                    "Name:".bold(),
                    user.name.as_deref().unwrap_or("(not set)")
                );
                println!(
                    "{} {}",
                    "Role:".bold(),
                    user.role.as_deref().unwrap_or("user")
                );
                Ok(())
            })
        }
        _ => Err(format!(
            "unknown skill action: '{}'. Use: list, info, install, search, update, publish, star, unstar, versions, doctor, whoami, enable, disable, remove",
            action
        )
        .into()),
    }
}

/// Primary global skills directory: `~/.synapse/skills/`
fn global_skills_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".synapse")
        .join("skills")
}

/// Legacy global skills directory: `~/.claude/skills/` (OpenClaw compat)
fn legacy_global_skills_dir() -> PathBuf {
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
    let mut version = None;
    let mut description = None;
    for line in lines {
        let trimmed = line.trim();
        if trimmed == "---" {
            break;
        }
        if let Some(val) = trimmed.strip_prefix("name:") {
            name = Some(val.trim().trim_matches('"').trim_matches('\'').to_string());
        } else if let Some(val) = trimmed.strip_prefix("version:") {
            version = Some(val.trim().trim_matches('"').trim_matches('\'').to_string());
        } else if let Some(val) = trimmed.strip_prefix("description:") {
            description = Some(val.trim().trim_matches('"').trim_matches('\'').to_string());
        }
    }
    Some(SkillInfo {
        name: name?,
        version: version.unwrap_or_else(|| "-".to_string()),
        description: description.unwrap_or_default(),
    })
}

fn list_skills() -> Result<(), Box<dyn std::error::Error>> {
    let global_dir = global_skills_dir();
    let legacy_dir = legacy_global_skills_dir();
    let workspace_dir = workspace_skills_dir();

    let global_skills = if global_dir.exists() {
        discover_all_skills(&global_dir)
    } else {
        Vec::new()
    };

    // Legacy ~/.claude/skills/ — only include skills not already in global
    let legacy_skills = if legacy_dir.exists() {
        let global_names: std::collections::HashSet<_> =
            global_skills.iter().map(|(_, i)| i.name.clone()).collect();
        discover_all_skills(&legacy_dir)
            .into_iter()
            .filter(|(_, i)| !global_names.contains(&i.name))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };

    let workspace_skills = workspace_dir
        .as_ref()
        .map(|d| discover_all_skills(d))
        .unwrap_or_default();

    if global_skills.is_empty() && legacy_skills.is_empty() && workspace_skills.is_empty() {
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

    for (path, info) in &legacy_skills {
        let status = if is_disabled(path) {
            "disabled".red().to_string()
        } else {
            "enabled".green().to_string()
        };
        println!(
            "{:<20} {:<10} {:<10} {:<10} {}",
            info.name, info.version, "compat", status, info.description
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

    let total = global_skills.len() + legacy_skills.len() + workspace_skills.len();
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
        v.push(("compat", legacy_global_skills_dir()));
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
                    println!("{} SKILL.md (OpenClaw)", "Format:".bold());
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
            let manifest = load_manifest(&manifest_path)
                .map_err(|e| format!("failed to load manifest: {}", e))?;

            println!("{} {}", "Name:".bold(), manifest.name);
            println!("{} {}", "Version:".bold(), manifest.version);
            println!("{} {}", "Description:".bold(), manifest.description);
            println!("{} manifest.toml (legacy)", "Format:".bold());
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
    println!("{} Skill '{}' enabled", "enable:".green().bold(), name);
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
    println!("{} Skill '{}' disabled", "disable:".yellow().bold(), name);
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

/// Publish a skill directory to ClawHub.
async fn publish_skill(
    hub: &crate::hub::ClawHubClient,
    source: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let source_path = Path::new(source);
    if !source_path.exists() || !source_path.is_dir() {
        return Err("source must be a directory containing SKILL.md".into());
    }

    let skill_md = source_path.join("SKILL.md");
    if !skill_md.exists() {
        return Err("no SKILL.md found in source directory".into());
    }

    let content = std::fs::read_to_string(&skill_md)?;
    let info = parse_skill_md_info(&content)
        .ok_or("SKILL.md missing required 'name' field in frontmatter")?;

    // Collect all non-hidden files
    let mut files: Vec<(String, Vec<u8>)> = Vec::new();
    collect_publish_files(source_path, source_path, &mut files)?;

    println!(
        "Publishing '{}' v{} ({} files)...",
        info.name,
        info.version,
        files.len()
    );

    let result = hub.publish(&info.name, &info.version, files).await?;
    if result.ok {
        println!(
            "{} Published '{}' v{} to ClawHub",
            "publish:".green().bold(),
            info.name,
            info.version
        );
    } else {
        println!("{} Publish failed", "error:".red().bold());
    }
    Ok(())
}

fn collect_publish_files(
    base: &Path,
    dir: &Path,
    out: &mut Vec<(String, Vec<u8>)>,
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
        if name.starts_with('.') {
            continue; // skip hidden files/dirs
        }
        if path.is_dir() {
            collect_publish_files(base, &path, out)?;
        } else {
            let rel = path.strip_prefix(base)?.to_string_lossy().to_string();
            let content = std::fs::read(&path)?;
            out.push((rel, content));
        }
    }
    Ok(())
}

/// Run health checks on all installed skills.
fn doctor_skills() -> Result<(), Box<dyn std::error::Error>> {
    let global_dir = global_skills_dir();
    let ws_dir = workspace_skills_dir();

    println!("{}", "Skill Health Check".bold());
    println!("{}", "=".repeat(60));

    let mut total = 0;
    let mut healthy = 0;
    let mut issues = 0;

    let dirs: Vec<(&str, PathBuf)> = {
        let mut v = Vec::new();
        v.push(("global", global_dir));
        v.push(("compat", legacy_global_skills_dir()));
        if let Some(ws) = ws_dir {
            v.push(("workspace", ws));
        }
        v
    };

    for (scope, dir) in &dirs {
        if !dir.exists() {
            continue;
        }
        for (path, info) in discover_all_skills(dir) {
            total += 1;
            let disabled = is_disabled(&path);
            let skill_md = path.join("SKILL.md");

            let mut skill_issues: Vec<String> = Vec::new();

            if disabled {
                skill_issues.push("disabled".to_string());
            }

            // Check required env vars / bins from frontmatter
            if let Ok(content) = std::fs::read_to_string(&skill_md) {
                check_frontmatter_requirements(&content, &mut skill_issues);
            }

            if skill_issues.is_empty() {
                healthy += 1;
                println!(
                    "  {} {} ({}) -- {}",
                    "ok".green(),
                    info.name,
                    scope,
                    "healthy".green()
                );
            } else {
                issues += 1;
                println!(
                    "  {} {} ({}) -- {}",
                    "!!".red(),
                    info.name,
                    scope,
                    skill_issues.join(", ").yellow()
                );
            }
        }
    }

    println!(
        "\n{} total, {} healthy, {} with issues",
        total, healthy, issues
    );
    Ok(())
}

fn check_frontmatter_requirements(content: &str, issues: &mut Vec<String>) {
    let mut in_fm = false;
    for line in content.lines() {
        let t = line.trim();
        if t == "---" {
            if in_fm {
                break;
            }
            in_fm = true;
            continue;
        }
        if !in_fm {
            continue;
        }

        if let Some(bins) = t
            .strip_prefix("required-bins:")
            .or_else(|| t.strip_prefix("required_bins:"))
        {
            for bin in bins.split(',').map(|s| {
                s.trim()
                    .trim_matches(|c| c == '"' || c == '\'' || c == '[' || c == ']')
            }) {
                if !bin.is_empty() && which::which(bin).is_err() {
                    issues.push(format!("missing bin: {}", bin));
                }
            }
        }
        if let Some(envs) = t
            .strip_prefix("required-env:")
            .or_else(|| t.strip_prefix("required_env:"))
        {
            for env_var in envs.split(',').map(|s| {
                s.trim()
                    .trim_matches(|c| c == '"' || c == '\'' || c == '[' || c == ']')
            }) {
                if !env_var.is_empty() && std::env::var(env_var).is_err() {
                    issues.push(format!("missing env: {}", env_var));
                }
            }
        }
    }
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

    let dir = legacy_global_skills_dir().join(name);
    if dir.exists() {
        return Ok(dir);
    }

    Err(format!("skill '{}' not found", name).into())
}
