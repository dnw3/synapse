use synaptic::core::SynapticError;

use super::DockerWorkspace;

/// Manages Docker container lifecycle for sandboxed execution.
///
/// Used when the `docker` feature is enabled and a `[docker]` config section
/// is present. Creates, mounts, and destroys containers for sandboxed agent work.
#[allow(dead_code)]
pub struct DockerManager;

#[allow(dead_code)]
impl DockerManager {
    /// Create a new Docker workspace container.
    pub async fn create_workspace(
        image: &str,
        work_dir: &str,
    ) -> Result<DockerWorkspace, SynapticError> {
        let output = tokio::process::Command::new("docker")
            .args([
                "run", "-d", "--rm", "-w", work_dir, image, "sleep", "infinity",
            ])
            .output()
            .await
            .map_err(|e| SynapticError::Tool(format!("docker run failed: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SynapticError::Tool(format!(
                "docker run failed: {}",
                stderr
            )));
        }

        let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(DockerWorkspace::new(container_id, work_dir.to_string()))
    }

    /// Create a workspace with volume mount from host.
    pub async fn create_workspace_with_mount(
        image: &str,
        host_dir: &str,
        container_dir: &str,
    ) -> Result<DockerWorkspace, SynapticError> {
        let mount = format!("{}:{}", host_dir, container_dir);
        let output = tokio::process::Command::new("docker")
            .args([
                "run",
                "-d",
                "--rm",
                "-v",
                &mount,
                "-w",
                container_dir,
                image,
                "sleep",
                "infinity",
            ])
            .output()
            .await
            .map_err(|e| SynapticError::Tool(format!("docker run failed: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SynapticError::Tool(format!(
                "docker run failed: {}",
                stderr
            )));
        }

        let container_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(DockerWorkspace::new(
            container_id,
            container_dir.to_string(),
        ))
    }

    /// Stop and remove a workspace container.
    pub async fn destroy_workspace(container_id: &str) -> Result<(), SynapticError> {
        let output = tokio::process::Command::new("docker")
            .args(["kill", container_id])
            .output()
            .await
            .map_err(|e| SynapticError::Tool(format!("docker kill failed: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(SynapticError::Tool(format!(
                "docker kill failed: {}",
                stderr
            )));
        }

        Ok(())
    }
}
