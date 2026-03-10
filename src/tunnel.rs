//! Tunnel/remote access support.
//!
//! Exposes the local Synapse web server to the internet via a reverse tunnel.
//! Supports: cloudflared (Cloudflare Tunnel), bore, and SSH tunneling.

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
}

/// Start a tunnel to expose the local port.
///
/// This spawns a child process and prints the public URL.
/// The function blocks until the tunnel process exits.
pub async fn start_tunnel(
    provider: &str,
    local_port: u16,
    remote_host: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    match provider {
        "cloudflared" | "cloudflare" => run_cloudflared(local_port).await,
        "bore" => run_bore(local_port).await,
        "ssh" => {
            let host = remote_host.ok_or("SSH tunnel requires --remote-host")?;
            run_ssh_tunnel(local_port, host).await
        }
        _ => Err(format!(
            "Unknown tunnel provider '{}'. Available: cloudflared, bore, ssh",
            provider
        )
        .into()),
    }
}

async fn run_cloudflared(port: u16) -> Result<(), Box<dyn std::error::Error>> {
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

async fn run_bore(port: u16) -> Result<(), Box<dyn std::error::Error>> {
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

async fn run_ssh_tunnel(
    local_port: u16,
    remote_host: &str,
) -> Result<(), Box<dyn std::error::Error>> {
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

/// Auto-detect available tunnel provider.
#[allow(dead_code)]
pub fn detect_provider() -> Option<&'static str> {
    // Check in order of preference
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
