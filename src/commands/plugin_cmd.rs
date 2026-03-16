//! Plugin management CLI commands.
//!
//! Manages plugins stored in `.synapse/plugins/` (workspace-local) and
//! `~/.synapse/plugins/` (global), with persistent enable/disable state kept
//! in `.synapse/plugins/state.json`.

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
            let name_or_path = arg.ok_or("usage: synapse plugin install <name|path>")?;
            plugin_install(name_or_path)
        }
        "create" => {
            let name = arg.ok_or("usage: synapse plugin create <name>")?;
            plugin_create(name)
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
        "search" => {
            let query = arg.unwrap_or("");
            plugin_search(query)
        }
        "update" => plugin_update(arg),
        _ => Err(format!(
            "unknown plugin action: '{}'. Use: list, install, create, enable, disable, remove, search, update",
            action
        )
        .into()),
    }
}

// ---------------------------------------------------------------------------
// Directory helpers
// ---------------------------------------------------------------------------

/// Workspace plugin directory: `.synapse/plugins/` relative to CWD.
fn workspace_plugins_dir() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join(".synapse")
        .join("plugins")
}

/// Global plugin directory: `~/.synapse/plugins/`.
fn global_plugins_dir() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("~"))
        .join(".synapse")
        .join("plugins")
}

/// Ensure the global plugins directory exists.
fn ensure_global_plugins_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
    let dir = global_plugins_dir();
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

/// Legacy alias: workspace plugins dir (used by enable/disable/state).
fn plugins_dir() -> PathBuf {
    workspace_plugins_dir()
}

/// Path to the plugin state file (workspace-local).
fn plugin_state_path() -> PathBuf {
    plugins_dir().join("state.json")
}

// ---------------------------------------------------------------------------
// State helpers
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// Manifest helper
// ---------------------------------------------------------------------------

#[derive(serde::Deserialize, Default)]
struct Manifest {
    #[serde(default)]
    name: String,
    #[serde(default)]
    version: String,
    #[serde(default)]
    description: String,
}

fn read_manifest(dir: &Path) -> Manifest {
    let manifest_path = dir.join("plugin.toml");
    if !manifest_path.exists() {
        return Manifest::default();
    }
    let contents = std::fs::read_to_string(&manifest_path).unwrap_or_default();
    toml::from_str(&contents).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

/// List all plugins discovered in workspace `.synapse/plugins/` and global
/// `~/.synapse/plugins/`.
fn plugin_list() -> Result<(), Box<dyn std::error::Error>> {
    let workspace_dir = workspace_plugins_dir();
    let global_dir = global_plugins_dir();

    let disabled = load_disabled_plugins();
    let mut found = 0usize;

    println!(
        "{:<24} {:<10} {:<10} {:<10} DESCRIPTION",
        "NAME", "VERSION", "STATUS", "SCOPE"
    );
    println!("{}", "-".repeat(80));

    // Helper closure: print plugins from a given directory with a scope label.
    let mut print_dir = |dir: &Path, scope: &str| {
        if !dir.exists() {
            return;
        }
        let Ok(entries) = std::fs::read_dir(dir) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if !path.is_dir() {
                continue;
            }
            let manifest_path = path.join("plugin.toml");
            if !manifest_path.exists() {
                continue;
            }
            let manifest = read_manifest(&path);
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
                "{:<24} {:<10} {:<10} {:<10} {}",
                name, manifest.version, status, scope, manifest.description
            );
            found += 1;
        }
    };

    print_dir(&workspace_dir, "workspace");
    print_dir(&global_dir, "global");

    if found == 0 {
        println!("{}", "No plugins found.".dimmed());
        println!();
        println!(
            "Install a plugin:  {}",
            "synapse plugin install <name|path>".cyan()
        );
        println!(
            "Create a plugin:   {}",
            "synapse plugin create <name>".cyan()
        );
    } else {
        println!("\n{} plugin(s) found", found);
    }

    Ok(())
}

/// Install a plugin from a local path or by name (placeholder for registry
/// download).
///
/// - If `name_or_path` is an existing directory with `plugin.toml` → copy into
///   workspace plugins dir (existing behaviour).
/// - Otherwise → create a placeholder entry in `~/.synapse/plugins/<name>/`
///   (registry download not yet implemented).
fn plugin_install(name_or_path: &str) -> Result<(), Box<dyn std::error::Error>> {
    let source_path = Path::new(name_or_path);

    // --- Local path install ---
    if source_path.exists() && source_path.is_dir() {
        let manifest_path = source_path.join("plugin.toml");
        if !manifest_path.exists() {
            return Err(format!(
                "no plugin.toml found in '{}'. A plugin directory must contain a plugin.toml manifest.",
                source_path.display()
            )
            .into());
        }

        #[derive(serde::Deserialize)]
        struct LocalManifest {
            name: String,
            version: String,
        }

        let contents = std::fs::read_to_string(&manifest_path)?;
        let manifest: LocalManifest =
            toml::from_str(&contents).map_err(|e| format!("invalid plugin.toml: {}", e))?;

        let dest = workspace_plugins_dir().join(&manifest.name);
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
        return Ok(());
    }

    // --- Name-based install (registry placeholder) ---
    let name = name_or_path;
    let global_dir = ensure_global_plugins_dir()?;
    let dest = global_dir.join(name);

    if dest.exists() {
        return Err(format!(
            "plugin '{}' already installed at {}. Remove it first.",
            name,
            dest.display()
        )
        .into());
    }

    std::fs::create_dir_all(&dest)?;

    let placeholder_manifest = format!(
        r#"[plugin]
name = "{name}"
version = "unknown"
description = "Installed via registry (placeholder — registry download not yet implemented)"

[runtime]
command = ""
args = []
transport = "stdio"

[capabilities]
tools = false
"#,
        name = name
    );
    std::fs::write(dest.join("plugin.toml"), &placeholder_manifest)?;

    println!(
        "{} Plugin '{}' placeholder created at {}",
        "install:".yellow().bold(),
        name,
        dest.display()
    );
    println!(
        "{}",
        "Note: registry download is not yet implemented. Configure registry URL in synapse.toml."
            .dimmed()
    );

    Ok(())
}

/// Mark a plugin as enabled in the state file.
fn plugin_enable(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    // Check workspace dir first, then global.
    let workspace_plugin = workspace_plugins_dir().join(name);
    let global_plugin = global_plugins_dir().join(name);
    if !workspace_plugin.exists() && !global_plugin.exists() {
        return Err(format!("plugin '{}' not found", name).into());
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
    let workspace_plugin = workspace_plugins_dir().join(name);
    let global_plugin = global_plugins_dir().join(name);
    if !workspace_plugin.exists() && !global_plugin.exists() {
        return Err(format!("plugin '{}' not found", name).into());
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

/// Scaffold a new plugin directory with a `plugin.toml` template.
fn plugin_create(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let plugin_dir = Path::new(name);

    if plugin_dir.exists() {
        return Err(format!(
            "directory '{}' already exists. Choose a different name or remove it first.",
            name
        )
        .into());
    }

    std::fs::create_dir_all(plugin_dir)?;

    let manifest_content = format!(
        r#"[plugin]
name = "{name}"
version = "0.1.0"
description = "A Synapse plugin"

[runtime]
command = "node"
args = ["index.js"]
transport = "stdio"

[capabilities]
tools = true
"#,
        name = name
    );

    std::fs::write(plugin_dir.join("plugin.toml"), &manifest_content)?;

    println!(
        "{} Plugin scaffold created at '{}'",
        "create:".green().bold(),
        plugin_dir.display()
    );
    println!(
        "  {} {}",
        "manifest:".dimmed(),
        plugin_dir.join("plugin.toml").display()
    );
    println!();
    println!(
        "Next steps:\n  1. Edit {}/plugin.toml\n  2. Add your plugin's entry point (e.g. index.js)\n  3. Run: synapse plugin install {}",
        name, name
    );

    Ok(())
}

/// Remove a plugin directory entirely (checks workspace then global dir).
fn plugin_remove(name: &str) -> Result<(), Box<dyn std::error::Error>> {
    let workspace_plugin = workspace_plugins_dir().join(name);
    let global_plugin = global_plugins_dir().join(name);

    let plugin_dir = if workspace_plugin.exists() {
        workspace_plugin
    } else if global_plugin.exists() {
        global_plugin
    } else {
        return Err(format!("plugin '{}' not found", name).into());
    };

    std::fs::remove_dir_all(&plugin_dir)?;

    let mut disabled = load_disabled_plugins();
    if disabled.remove(name) {
        save_disabled_plugins(&disabled)?;
    }

    println!(
        "{} Plugin '{}' removed from {}",
        "remove:".red().bold(),
        name,
        plugin_dir.display()
    );
    Ok(())
}

/// Search the plugin registry (placeholder — registry not yet configured).
fn plugin_search(query: &str) -> Result<(), Box<dyn std::error::Error>> {
    if !query.is_empty() {
        println!("Searching for: {}", query.cyan());
    }
    println!(
        "{}",
        "Registry search not yet configured. Configure registry URL in synapse.toml.".yellow()
    );
    Ok(())
}

/// Check for plugin updates (placeholder — registry not yet configured).
fn plugin_update(name: Option<&str>) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(n) = name {
        println!("Checking updates for: {}", n.cyan());
    }
    println!(
        "{}",
        "Update check requires registry URL. Configure registry URL in synapse.toml.".yellow()
    );
    Ok(())
}
