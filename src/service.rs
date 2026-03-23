//! Service installation — generates systemd unit or launchd plist files.

use std::path::PathBuf;

/// Generate and optionally install a service configuration.
pub fn install_service(config_path: Option<&str>) -> crate::error::Result<()> {
    #[cfg(target_os = "linux")]
    {
        install_systemd(config_path)
    }

    #[cfg(target_os = "macos")]
    {
        install_launchd(config_path)
    }

    #[cfg(not(any(target_os = "linux", target_os = "macos")))]
    {
        let _ = config_path;
        Err("Service installation is only supported on Linux (systemd) and macOS (launchd)".into())
    }
}

#[cfg(target_os = "linux")]
fn install_systemd(config_path: Option<&str>) -> crate::error::Result<()> {
    let binary = std::env::current_exe()?;
    let config_flag = config_path
        .map(|p| format!(" --config {}", p))
        .unwrap_or_default();

    let unit = format!(
        r#"[Unit]
Description=Synapse AI Agent Server
After=network.target

[Service]
Type=simple
ExecStart={}{} serve
Restart=on-failure
RestartSec=5
Environment=RUST_LOG=info

[Install]
WantedBy=multi-user.target
"#,
        binary.display(),
        config_flag
    );

    let unit_path = PathBuf::from("/etc/systemd/system/synapse.service");
    println!("Generated systemd unit file:");
    println!("{}", unit);
    println!("\nTo install, run:");
    println!("  sudo tee {} <<'EOF'\n{}EOF", unit_path.display(), unit);
    println!("  sudo systemctl daemon-reload");
    println!("  sudo systemctl enable --now synapse");

    Ok(())
}

#[cfg(target_os = "macos")]
fn install_launchd(config_path: Option<&str>) -> crate::error::Result<()> {
    let binary = std::env::current_exe()?;
    let config_flag = config_path
        .map(|p| {
            format!(
                "\n    <string>--config</string>\n    <string>{}</string>",
                p
            )
        })
        .unwrap_or_default();

    let plist = format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.synapse.agent</string>
    <key>ProgramArguments</key>
    <array>
        <string>{}</string>{}
        <string>serve</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
    <key>StandardOutPath</key>
    <string>/tmp/synapse.log</string>
    <key>StandardErrorPath</key>
    <string>/tmp/synapse.err</string>
</dict>
</plist>
"#,
        binary.display(),
        config_flag
    );

    let plist_path = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("Library/LaunchAgents/com.synapse.agent.plist");

    println!("Generated launchd plist:");
    println!("{}", plist);
    println!("\nTo install, run:");
    println!("  tee {} <<'EOF'\n{}EOF", plist_path.display(), plist);
    println!("  launchctl load {}", plist_path.display());

    Ok(())
}
