//! VikingService — managed lifecycle for the openviking-server process.
//!
//! Implements the [`Service`] trait so the plugin system can start, health-check,
//! and stop the OpenViking background process automatically.

use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use synaptic::core::SynapticError;
use synaptic::plugin::Service;

use crate::memory::VikingConfig;

/// Managed lifecycle wrapper around the `openviking-server` subprocess.
pub struct VikingService {
    config: VikingConfig,
    process: Mutex<Option<tokio::process::Child>>,
}

impl VikingService {
    pub fn new(config: VikingConfig) -> Self {
        Self {
            config,
            process: Mutex::new(None),
        }
    }

    /// Build the health-check URL from the configured base URL.
    fn health_url(&self) -> String {
        format!("{}/health", self.config.url.trim_end_matches('/'))
    }
}

impl Drop for VikingService {
    fn drop(&mut self) {
        // Best-effort: kill the child process to avoid orphans.
        if let Ok(mut guard) = self.process.lock() {
            if let Some(child) = guard.as_mut() {
                let _ = child.start_kill();
            }
        }
    }
}

#[async_trait]
impl Service for VikingService {
    fn id(&self) -> &str {
        "memory-viking"
    }

    async fn start(&self) -> Result<(), SynapticError> {
        // Skip startup if the server is already reachable.
        if self.health_check().await {
            tracing::info!("openviking-server already running, skipping startup");
            return Ok(());
        }

        // Verify that the binary is available on PATH.
        let which_result = tokio::process::Command::new("which")
            .arg("openviking-server")
            .output()
            .await;

        match which_result {
            Ok(out) if out.status.success() => {
                tracing::info!(
                    binary = String::from_utf8_lossy(&out.stdout).trim(),
                    "found openviking-server"
                );
            }
            _ => {
                return Err(SynapticError::Tool(
                    "openviking-server not found on PATH; install with `pip install openviking`"
                        .into(),
                ));
            }
        }

        // Spawn the process and keep it alive for the service lifetime.
        let child = tokio::process::Command::new("openviking-server")
            .kill_on_drop(true)
            .spawn()
            .map_err(|e| SynapticError::Tool(format!("failed to spawn openviking-server: {e}")))?;

        {
            let mut guard = self
                .process
                .lock()
                .map_err(|_| SynapticError::Tool("VikingService process lock poisoned".into()))?;
            *guard = Some(child);
        }

        tracing::info!("openviking-server spawned, waiting for health check (up to 30s)");

        // Wait up to 30s for the server to become ready (Viking can be slow to initialize).
        let deadline = std::time::Instant::now() + Duration::from_secs(30);
        loop {
            if self.health_check().await {
                tracing::info!("openviking-server is healthy");
                return Ok(());
            }
            if std::time::Instant::now() >= deadline {
                return Err(SynapticError::Tool(
                    "openviking-server did not become healthy within 30s".into(),
                ));
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    async fn health_check(&self) -> bool {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap_or_default();

        match client.get(self.health_url()).send().await {
            Ok(resp) => resp.status().is_success(),
            Err(_) => false,
        }
    }

    async fn stop(&self) {
        let child_opt = {
            let mut guard = match self.process.lock() {
                Ok(g) => g,
                Err(_) => return,
            };
            guard.take()
        };

        if let Some(mut child) = child_opt {
            // Send SIGKILL; ignore errors (process may have already exited).
            let _ = child.start_kill();

            // Wait up to 5 s for the process to exit.
            let timeout = tokio::time::timeout(Duration::from_secs(5), child.wait());
            match timeout.await {
                Ok(Ok(_)) => tracing::info!("openviking-server stopped"),
                Ok(Err(e)) => tracing::warn!(error = %e, "error waiting for openviking-server"),
                Err(_) => tracing::warn!("openviking-server did not exit within 5s"),
            }
        }
    }
}
