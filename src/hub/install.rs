//! Skill installation logic for ClawHub skills.

use std::collections::HashMap;
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

/// Validate a skill slug: must be non-empty, no path separators or traversal.
fn validate_slug(slug: &str) -> Result<(), Box<dyn std::error::Error>> {
    if slug.is_empty()
        || slug.contains('/')
        || slug.contains('\\')
        || slug.contains("..")
        || slug.contains('\0')
    {
        return Err(format!("invalid skill slug: {}", slug).into());
    }
    if !slug
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
    {
        return Err(format!("invalid characters in skill slug: {}", slug).into());
    }
    Ok(())
}

/// Install a skill from ClawHub into the global skills directory.
pub async fn install_from_hub(
    hub: &super::ClawHubClient,
    slug: &str,
    version: Option<&str>,
    upgrade: bool,
) -> Result<(), Box<dyn std::error::Error>> {
    validate_slug(slug)?;

    let global_dir = global_skills_dir();
    std::fs::create_dir_all(&global_dir)?;

    let target = global_dir.join(slug);
    if target.exists() && !upgrade {
        return Err(format!(
            "skill '{}' already installed at {}. Use --upgrade to overwrite.",
            slug,
            target.display()
        )
        .into());
    }

    tracing::info!(package = %slug, version = ?version, "installing from hub");

    // Download skill zip
    let zip_bytes = hub.download_zip(slug, version).await?;

    // Create/overwrite target directory
    if target.exists() {
        std::fs::remove_dir_all(&target)?;
    }
    std::fs::create_dir_all(&target)?;

    // Extract zip contents (with Zip Slip protection)
    let reader = std::io::Cursor::new(&zip_bytes);
    let mut archive = zip::ZipArchive::new(reader)?;
    let canonical_target = std::fs::canonicalize(&target)?;
    for i in 0..archive.len() {
        let mut file = archive.by_index(i)?;
        let name = file.name().to_string();

        // Reject entries with path traversal components
        if name.contains("..") {
            return Err(format!("zip path traversal attempt: {}", name).into());
        }

        let out_path = target.join(&name);

        // Skip directories and hidden files
        if name.ends_with('/') {
            std::fs::create_dir_all(&out_path)?;
            // Verify the created directory is within target
            let canonical = std::fs::canonicalize(&out_path)?;
            if !canonical.starts_with(&canonical_target) {
                return Err(format!("zip path traversal attempt: {}", name).into());
            }
            continue;
        }

        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Verify output path is within target directory
        let canonical_parent = std::fs::canonicalize(out_path.parent().unwrap_or(&target))?;
        if !canonical_parent.starts_with(&canonical_target) {
            return Err(format!("zip path traversal attempt: {}", name).into());
        }

        let mut out_file = std::fs::File::create(&out_path)?;
        std::io::copy(&mut file, &mut out_file)?;
    }

    // Determine installed version from SKILL.md frontmatter or fallback
    let installed_version = version.unwrap_or("latest").to_string();

    // Write install manifest
    let manifest = InstalledManifest {
        name: slug.to_string(),
        version: installed_version.clone(),
        source: format!("clawhub:{}", slug),
        installed_at: chrono_now(),
    };
    let manifest_json = serde_json::to_string_pretty(&manifest)?;
    std::fs::write(target.join("manifest.json"), manifest_json)?;

    tracing::info!(
        slug = %slug,
        version = %installed_version,
        path = %target.display(),
        "installed skill from hub"
    );
    println!(
        "{} Installed skill '{}' v{} from ClawHub to {}",
        "install:".green().bold(),
        slug,
        installed_version,
        target.display()
    );

    // Check requirements if the SKILL.md specifies them
    let skill_md_path = target.join("SKILL.md");
    if skill_md_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&skill_md_path) {
            check_requirements(&content);
        }
    }

    // Update lock file
    let fingerprint = compute_fingerprint(&target).ok();
    if let Err(e) = update_lock_entry(slug, Some(&installed_version), fingerprint.as_deref()) {
        tracing::warn!(error = %e, "failed to update lock file");
    }

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
            let bins: Vec<&str> = bins
                .split(',')
                .map(|s| s.trim().trim_matches('"'))
                .collect();
            for bin in bins {
                if !bin.is_empty() && which::which(bin).is_err() {
                    tracing::warn!(binary = %bin, "required binary not found in PATH");
                }
            }
        }
        if let Some(envs) = trimmed.strip_prefix("required-env:") {
            let envs: Vec<&str> = envs
                .split(',')
                .map(|s| s.trim().trim_matches('"'))
                .collect();
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
        .join(".synapse")
        .join("skills")
}

fn chrono_now() -> String {
    use std::time::{SystemTime, UNIX_EPOCH};
    let dur = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    format!("{}", dur.as_secs())
}

// ---------------------------------------------------------------------------
// Lock file for tracking installed skills
// ---------------------------------------------------------------------------

/// Lock file for tracking installed skills.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct LockFile {
    pub version: String,
    pub skills: HashMap<String, LockEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LockEntry {
    pub version: Option<String>,
    #[serde(rename = "installedAt")]
    pub installed_at: u64,
    pub fingerprint: Option<String>,
}

/// Compute fingerprint (SHA256 of sorted file paths + contents hashes).
pub fn compute_fingerprint(dir: &Path) -> Result<String, Box<dyn std::error::Error>> {
    use sha2::{Digest, Sha256};

    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    collect_files(dir, dir, &mut entries)?;
    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let mut hasher = Sha256::new();
    for (path, content) in &entries {
        hasher.update(path.as_bytes());
        let mut file_hasher = Sha256::new();
        file_hasher.update(content);
        hasher.update(file_hasher.finalize());
    }

    Ok(format!("{:x}", hasher.finalize()))
}

fn collect_files(
    base: &Path,
    dir: &Path,
    out: &mut Vec<(String, Vec<u8>)>,
) -> Result<(), Box<dyn std::error::Error>> {
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            if path
                .file_name()
                .map(|n| n.to_str().unwrap_or(""))
                .unwrap_or("")
                == ".clawhub"
            {
                continue; // skip metadata dir
            }
            collect_files(base, &path, out)?;
        } else {
            let rel = path.strip_prefix(base)?.to_string_lossy().to_string();
            let content = std::fs::read(&path)?;
            out.push((rel, content));
        }
    }
    Ok(())
}

/// Read the lock file from the skills directory.
pub fn read_lock_file() -> LockFile {
    let path = global_skills_dir().join(".clawhub").join("lock.json");
    if let Ok(content) = std::fs::read_to_string(&path) {
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        LockFile::default()
    }
}

/// Write the lock file to the skills directory.
pub fn write_lock_file(lock: &LockFile) -> Result<(), Box<dyn std::error::Error>> {
    let dir = global_skills_dir().join(".clawhub");
    std::fs::create_dir_all(&dir)?;
    let content = serde_json::to_string_pretty(lock)?;
    std::fs::write(dir.join("lock.json"), content)?;
    Ok(())
}

/// Update the lock file entry for a skill.
pub fn update_lock_entry(
    name: &str,
    version: Option<&str>,
    fingerprint: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut lock = read_lock_file();
    if lock.version.is_empty() {
        lock.version = "1".to_string();
    }
    lock.skills.insert(
        name.to_string(),
        LockEntry {
            version: version.map(String::from),
            installed_at: std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_millis() as u64,
            fingerprint: fingerprint.map(String::from),
        },
    );
    write_lock_file(&lock)
}
