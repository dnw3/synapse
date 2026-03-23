//! Tunnel/remote access support.
//!
//! Exposes the local Synapse web server to the internet via a reverse tunnel.
//! Supports: cloudflared (Cloudflare Tunnel), bore, SSH tunneling, and Tailscale Funnel.

use colored::Colorize;

/// Tunnel provider type.
#[allow(dead_code)]
pub enum TunnelProvider {
    /// Cloudflare Tunnel (requires `cloudflared` binary).
    Cloudflared,
    /// Bore tunnel (requires `bore` binary).
    Bore,
    /// SSH reverse tunnel.
    Ssh { host: String },
    /// Tailscale Funnel (requires `tailscale` binary with funnel enabled).
    Tailscale,
}

/// Start a tunnel to expose the local port.
///
/// This spawns a child process and prints the public URL.
/// The function blocks until the tunnel process exits.
pub async fn start_tunnel(
    provider: &str,
    local_port: u16,
    remote_host: Option<&str>,
) -> crate::error::Result<()> {
    match provider {
        "cloudflared" | "cloudflare" => run_cloudflared(local_port).await,
        "bore" => run_bore(local_port).await,
        "ssh" => {
            let host = remote_host.ok_or("SSH tunnel requires --remote-host")?;
            run_ssh_tunnel(local_port, host).await
        }
        "tailscale" => run_tailscale_funnel(local_port).await,
        _ => Err(format!(
            "Unknown tunnel provider '{}'. Available: cloudflared, bore, ssh, tailscale",
            provider
        )
        .into()),
    }
}

async fn run_cloudflared(port: u16) -> crate::error::Result<()> {
    // Check if cloudflared is available
    let which = tokio::process::Command::new("which")
        .arg("cloudflared")
        .output()
        .await;

    if which.is_err() || !which.unwrap().status.success() {
        return Err(
            "cloudflared not found. Install: https://developers.cloudflare.com/cloudflare-one/connections/connect-networks/downloads/"
                .into(),
        );
    }

    eprintln!(
        "{} Starting Cloudflare tunnel to localhost:{}...",
        "tunnel:".cyan().bold(),
        port
    );

    let mut child = tokio::process::Command::new("cloudflared")
        .args(["tunnel", "--url", &format!("http://localhost:{}", port)])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()?;

    let status = child.wait().await?;
    if !status.success() {
        return Err("cloudflared exited with error".into());
    }
    Ok(())
}

async fn run_bore(port: u16) -> crate::error::Result<()> {
    let which = tokio::process::Command::new("which")
        .arg("bore")
        .output()
        .await;

    if which.is_err() || !which.unwrap().status.success() {
        return Err("bore not found. Install: cargo install bore-cli".into());
    }

    eprintln!(
        "{} Starting bore tunnel to localhost:{}...",
        "tunnel:".cyan().bold(),
        port
    );

    let mut child = tokio::process::Command::new("bore")
        .args(["local", &port.to_string(), "--to", "bore.pub"])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()?;

    let status = child.wait().await?;
    if !status.success() {
        return Err("bore exited with error".into());
    }
    Ok(())
}

async fn run_ssh_tunnel(local_port: u16, remote_host: &str) -> crate::error::Result<()> {
    eprintln!(
        "{} Starting SSH tunnel: localhost:{} -> {}",
        "tunnel:".cyan().bold(),
        local_port,
        remote_host
    );

    // ssh -R 80:localhost:$PORT remote_host
    let mut child = tokio::process::Command::new("ssh")
        .args([
            "-R",
            &format!("0:localhost:{}", local_port),
            remote_host,
            "-N", // don't execute a remote command
        ])
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()?;

    let status = child.wait().await?;
    if !status.success() {
        return Err("SSH tunnel exited with error".into());
    }
    Ok(())
}

async fn run_tailscale_funnel(port: u16) -> crate::error::Result<()> {
    // Check if tailscale is available
    let which = tokio::process::Command::new("which")
        .arg("tailscale")
        .output()
        .await;

    if which.is_err() || !which.unwrap().status.success() {
        return Err("tailscale not found. Install: https://tailscale.com/download".into());
    }

    eprintln!(
        "{} Starting Tailscale funnel on port {}...",
        "tunnel:".cyan().bold(),
        port
    );

    // Enable the funnel in the background
    let funnel_output = tokio::process::Command::new("tailscale")
        .args(["funnel", &port.to_string(), "--bg"])
        .output()
        .await?;

    if !funnel_output.status.success() {
        let stderr = String::from_utf8_lossy(&funnel_output.stderr);
        return Err(format!("tailscale funnel failed: {}", stderr).into());
    }

    // Retrieve the Tailscale node's DNS name to construct the public URL
    let status_output = tokio::process::Command::new("tailscale")
        .args(["status", "--json"])
        .output()
        .await?;

    let url = if status_output.status.success() {
        let status: serde_json::Value =
            serde_json::from_slice(&status_output.stdout).unwrap_or(serde_json::Value::Null);
        let dns_name = status
            .get("Self")
            .and_then(|s| s.get("DNSName"))
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .trim_end_matches('.');
        format!("https://{}", dns_name)
    } else {
        "https://<your-tailscale-node>".to_string()
    };

    eprintln!(
        "{} Tailscale funnel active: {}",
        "tunnel:".cyan().bold(),
        url
    );
    eprintln!(
        "{} Press Ctrl-C to stop the funnel.",
        "tunnel:".cyan().bold()
    );

    // Keep the process alive; funnel runs as a background daemon via tailscale
    // Wait for a signal to shut down
    tokio::signal::ctrl_c().await?;

    // Disable funnel on exit
    eprintln!("{} Stopping Tailscale funnel...", "tunnel:".cyan().bold());
    let _ = tokio::process::Command::new("tailscale")
        .args(["funnel", "--reset"])
        .output()
        .await;

    Ok(())
}

/// Expose the local port via Tailscale Funnel and return the public URL.
///
/// This is a non-blocking variant that starts the funnel in the background
/// and returns the URL immediately.
#[allow(dead_code)]
pub async fn setup_tailscale_funnel(port: u16) -> crate::error::Result<String> {
    let output = tokio::process::Command::new("tailscale")
        .args(["funnel", &port.to_string(), "--bg"])
        .output()
        .await?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("tailscale funnel failed: {}", stderr).into());
    }

    // Get the hostname
    let hostname_output = tokio::process::Command::new("tailscale")
        .args(["status", "--json"])
        .output()
        .await?;

    let status: serde_json::Value = serde_json::from_slice(&hostname_output.stdout)?;
    let dns_name = status
        .get("Self")
        .and_then(|s| s.get("DNSName"))
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .trim_end_matches('.');

    Ok(format!("https://{}", dns_name))
}

/// Auto-detect available tunnel provider.
#[allow(dead_code)]
pub fn detect_provider() -> Option<&'static str> {
    // Check in order of preference
    if std::process::Command::new("tailscale")
        .arg("--version")
        .output()
        .is_ok()
    {
        return Some("tailscale");
    }
    if std::process::Command::new("cloudflared")
        .arg("--version")
        .output()
        .is_ok()
    {
        return Some("cloudflared");
    }
    if std::process::Command::new("bore")
        .arg("--version")
        .output()
        .is_ok()
    {
        return Some("bore");
    }
    None
}
