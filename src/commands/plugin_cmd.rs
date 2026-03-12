//! Plugin management CLI commands.
//!
//! Manages plugins stored in `.synapse/plugins/`, with persistent
//! enable/disable state kept in `.synapse/plugins/state.json`.

use std::path::{Path, PathBuf};

use colored::Colorize;

/// Run plugin subcommand.
pub fn run_plugin_command(
    action: &str,
    arg: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    match action {
        "list" | "ls" => plugin_list(),
        "install" => {
            let source = arg.ok_or("usage: synapse plugin install <path>")?;
            plugin_install(source)
        }
        "enable" => {
            let name = arg.ok_or("usage: synapse plugin enable <name>")?;
            plugin_enable(name)
        }
        "disable" => {
            let name = arg.ok_or("usage: synapse plugin disable <name>")?;
            plugin_disable(name)
        }
        "remove" | "rm" => {
            let name = arg.ok_or("usage: synapse plugin remove <name>")?;
            plugin_remove(name)
        }
        _ => Err(format!(
            "unknown plugin action: '{}'. Use: list, install, enable, disable, remove",
            action
        )
        .into()),
    }
}

/// Workspace plugin directory: `.synapse/plugins/` relative to CWD.
fn plugins_dir() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".synapse")
        .join("plugins")
}

/// Path to the plugin state file.
fn plugin_state_path() -> PathBuf {
    plugins_dir().join("state.json")
}

/// Load the set of disabled plugin names from `state.json`.
fn load_disabled_plugins() -> std::collections::HashSet<String> {
    let path = plugin_state_path();
    if !path.exists() {
        return std::collections::HashSet::new();
    }

    let contents = match std::fs::read_to_string(&path) {
        Ok(s) => s,
        Err(_) => return std::collections::HashSet::new(),
    };

    #[derive(serde::Deserialize)]
    struct State {
        #[serde(default)]
        disabled: Vec<String>,
    }

    match serde_json::from_str::<State>(&contents) {
        Ok(s) => s.disabled.into_iter().collect(),
        Err(_) => std::collections::HashSet::new(),
    }
}

/// Persist the disabled-plugin set back to `state.json`.
fn save_disabled_plugins(
    disabled: &std::collections::HashSet<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let path = plugin_state_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut sorted: Vec<&str> = disabled.iter().map(String::as_str).collect();
    sorted.sort_unstable();

    let json = serde_json::json!({ "disabled": sorted });
    std::fs::write(&path, serde_json::to_string_pretty(&json)?)?;
    Ok(())
}

/// List all plugins discovered in `.synapse/plugins/`.
fn plugin_list() -> Result<(), Box<dyn std::error::Error>> {
    let dir = plugins_dir();

    if !dir.exists() {
        println!("{}", "No plugins directory found.".dimmed());
        println!("  Expected: {}", dir.display());
        println!(
            "\nInstall a plugin: {}",
            "synapse plugin install <path>".cyan()
        );
        return Ok(());
    }

    let disabled = load_disabled_plugins();
    let mut found = 0usize;

    println!(
        "{:<24} {:<10} {:<10} DESCRIPTION",
        "NAME", "VERSION", "STATUS"
    );
    println!("{}", "-".repeat(70));

    for entry in std::fs::read_dir(&dir)?.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let manifest_path = path.join("plugin.toml");
        if !manifest_path.exists() {
            continue;
        }

        let contents = std::fs::read_to_string(&manifest_path).unwrap_or_default();

        #[derive(serde::Deserialize, Default)]
        struct Manifest {
            #[serde(default)]
            name: String,
            #[serde(default)]
            version: String,
            #[serde(default)]
            description: String,
        }

        let manifest: Manifest = toml::from_str(&contents).unwrap_or_default();
        let name = if manifest.name.is_empty() {
            entry.file_name().to_string_lossy().into_owned()
        } else {
            manifest.name.clone()
        };

        let status = if disabled.contains(&name) {
            "disabled".red().to_string()
        } else {
            "enabled".green().to_string()
        };

        println!(
            "{:<24} {:<10} {:<10} {}",
            name, manifest.version, status, manifest.description
        );
        found += 1;
    }

    if found == 0 {
        println!("{}", "No plugins found.".dimmed());
    } else {
        println!("\n{} plugin(s) found", found);
    }

    Ok(())
}

/// Install a plugin from a local path.
fn plugin_install(source: &str) -> Result<(), Box<dyn std::error::Error>> {
    let source_path = Path::new(source);
    if !source_path.exists() {
        return Err(format!("source path '{}' does not exist", source).into());
    }

    let manifest_path = source_path.join("plugin.toml");
    if !manifest_path.exists() {
        return Err(format!(
            "no plugin.toml found in '{}'. A plugin directory must contain a plugin.toml manifest.",
            source_path.display()
        )
        .into());
    }

    #[derive(serde::Deserialize)]
    struct Manifest {
        name: String,
        version: String,
    }

    let contents = std::fs::read_to_string(&manifest_path)?;
    let manifest: Manifest =
        toml::from_str(&contents).map_err(|e| format!("invalid plugin.toml: {}", e))?;

    let dest = plugins_dir().join(&manifest.name);
    if dest.exists() {
        return Err(format!(
            "plugin '{}' already installed at {}. Remove it first.",
            manifest.name,
            dest.display()
        )
        .into());
    }

    std::fs::create_dir_all(&dest)?;
    super::copy_dir_recursive(source_path, &dest)?;

    println!(
        "{} Plugin '{}' v{} installed to {}",
        "install:".green().bold(),
        manifest.name,
        manifest.version,
        dest.display()
    );
    println!(
        "{}",
        "Note: dynamic loading is not yet implemented. Restart Synapse to pick up new plugins."
            .dimmed()
    );
    Ok(())
}

/// Mark a plugin as enabled in the state file.
fn plugin_enable(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let plugin_dir = plugins_dir().join(name);
    if !plugin_dir.exists() {
        return Err(format!("plugin '{}' not found in {}", name, plugins_dir().display()).into());
    }

    let mut disabled = load_disabled_plugins();

    if !disabled.contains(name) {
        println!("Plugin '{}' is already enabled.", name);
        return Ok(());
    }

    disabled.remove(name);
    save_disabled_plugins(&disabled)?;

    println!("{} Plugin '{}' enabled", "enable:".green().bold(), name);
    Ok(())
}

/// Mark a plugin as disabled in the state file.
fn plugin_disable(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let plugin_dir = plugins_dir().join(name);
    if !plugin_dir.exists() {
        return Err(format!("plugin '{}' not found in {}", name, plugins_dir().display()).into());
    }

    let mut disabled = load_disabled_plugins();

    if disabled.contains(name) {
        println!("Plugin '{}' is already disabled.", name);
        return Ok(());
    }

    disabled.insert(name.to_string());
    save_disabled_plugins(&disabled)?;

    println!("{} Plugin '{}' disabled", "disable:".yellow().bold(), name);
    Ok(())
}

/// Remove a plugin directory entirely.
fn plugin_remove(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let plugin_dir = plugins_dir().join(name);
    if !plugin_dir.exists() {
        return Err(format!("plugin '{}' not found in {}", name, plugins_dir().display()).into());
    }

    std::fs::remove_dir_all(&plugin_dir)?;

    let mut disabled = load_disabled_plugins();
    if disabled.remove(name) {
        save_disabled_plugins(&disabled)?;
    }

    println!(
        "{} Plugin '{}' removed from {}",
        "remove:".red().bold(),
        name,
        plugins_dir().display()
    );
    Ok(())
}
