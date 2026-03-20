pub mod manager;

use async_trait::async_trait;
use std::time::Duration;
use synaptic::core::SynapticError;
use synaptic::deep::backend::{Backend, DirEntry, ExecResult, GrepOutputMode};

/// Docker-backed workspace for sandboxed agent execution.
///
/// All file and command operations are executed inside a Docker container
/// via `docker exec`. The container must be started via [`manager::DockerManager`].
pub struct DockerWorkspace {
    container_id: String,
    work_dir: String,
}

impl DockerWorkspace {
    pub fn new(container_id: String, work_dir: String) -> Self {
        Self {
            container_id,
            work_dir,
        }
    }

    /// Execute a command inside the container and return stdout.
    async fn docker_exec(&self, cmd: &str) -> Result<ExecResult, SynapticError> {
        let output = tokio::process::Command::new("docker")
            .args([
                "exec",
                "-w",
                &self.work_dir,
                &self.container_id,
                "sh",
                "-c",
                cmd,
            ])
            .output()
            .await
            .map_err(|e| SynapticError::Tool(format!("docker exec failed: {}", e)))?;

        Ok(ExecResult {
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            exit_code: output.status.code().unwrap_or(-1),
        })
    }
}

#[async_trait]
impl Backend for DockerWorkspace {
    async fn ls(&self, path: &str) -> Result<Vec<DirEntry>, SynapticError> {
        let cmd = format!("ls -la --time-style=+%s {}", shell_escape(path));
        let result = self.docker_exec(&cmd).await?;

        if result.exit_code != 0 {
            return Err(SynapticError::Tool(format!("ls failed: {}", result.stderr)));
        }

        let mut entries = Vec::new();
        for line in result.stdout.lines().skip(1) {
            // Parse ls -la output
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() < 7 {
                continue;
            }
            let perms = parts[0];
            let size: u64 = parts[4].parse().unwrap_or(0);
            let name = parts[6..].join(" ");

            if name == "." || name == ".." {
                continue;
            }

            entries.push(DirEntry {
                name,
                is_dir: perms.starts_with('d'),
                size: Some(size),
            });
        }

        Ok(entries)
    }

    async fn read_file(
        &self,
        path: &str,
        offset: usize,
        limit: usize,
    ) -> Result<String, SynapticError> {
        let start = offset + 1;
        let end = offset + limit;
        let cmd = format!("sed -n '{},{}p' {}", start, end, shell_escape(path));
        let result = self.docker_exec(&cmd).await?;

        if result.exit_code != 0 {
            return Err(SynapticError::Tool(format!(
                "read failed: {}",
                result.stderr
            )));
        }

        Ok(result.stdout)
    }

    async fn write_file(&self, path: &str, content: &str) -> Result<(), SynapticError> {
        // Create parent directories
        if let Some(parent) = std::path::Path::new(path).parent() {
            let mkdir_cmd = format!("mkdir -p {}", shell_escape(&parent.to_string_lossy()));
            self.docker_exec(&mkdir_cmd).await?;
        }

        // Write via stdin through docker exec
        let mut child = tokio::process::Command::new("docker")
            .args([
                "exec",
                "-i",
                "-w",
                &self.work_dir,
                &self.container_id,
                "sh",
                "-c",
                &format!("cat > {}", shell_escape(path)),
            ])
            .stdin(std::process::Stdio::piped())
            .spawn()
            .map_err(|e| SynapticError::Tool(format!("docker exec failed: {}", e)))?;

        if let Some(mut stdin) = child.stdin.take() {
            use tokio::io::AsyncWriteExt;
            stdin
                .write_all(content.as_bytes())
                .await
                .map_err(|e| SynapticError::Tool(format!("write failed: {}", e)))?;
        }

        let status = child
            .wait()
            .await
            .map_err(|e| SynapticError::Tool(format!("docker exec wait: {}", e)))?;

        if !status.success() {
            return Err(SynapticError::Tool("write_file failed in container".into()));
        }

        Ok(())
    }

    async fn edit_file(
        &self,
        path: &str,
        old_text: &str,
        new_text: &str,
        _replace_all: bool,
    ) -> Result<(), SynapticError> {
        // Read, replace, write
        let content = self.read_file(path, 0, 100_000).await?;
        if !content.contains(old_text) {
            return Err(SynapticError::Tool(format!(
                "old_string not found in {}",
                path
            )));
        }
        let new_content = content.replacen(old_text, new_text, 1);
        self.write_file(path, &new_content).await
    }

    async fn glob(&self, pattern: &str, base: &str) -> Result<Vec<String>, SynapticError> {
        let cmd = format!(
            "find {} -type f -name '{}' 2>/dev/null | sort",
            shell_escape(base),
            pattern.replace("**", "*")
        );
        let result = self.docker_exec(&cmd).await?;

        Ok(result
            .stdout
            .lines()
            .filter(|l| !l.is_empty())
            .map(|l| l.to_string())
            .collect())
    }

    async fn grep(
        &self,
        pattern: &str,
        path: Option<&str>,
        file_glob: Option<&str>,
        output_mode: GrepOutputMode,
    ) -> Result<String, SynapticError> {
        let dir = path.unwrap_or(".");
        let mut cmd = String::from("grep -r");

        match output_mode {
            GrepOutputMode::FilesWithMatches => cmd.push_str(" -l"),
            GrepOutputMode::Count => cmd.push_str(" -c"),
            GrepOutputMode::Content => cmd.push_str(" -n"),
        }

        if let Some(glob) = file_glob {
            cmd.push_str(&format!(" --include='{}'", glob));
        }

        cmd.push_str(&format!(" {} {}", shell_escape(pattern), shell_escape(dir)));
        cmd.push_str(" 2>/dev/null");

        let result = self.docker_exec(&cmd).await?;
        Ok(result.stdout)
    }

    async fn execute(
        &self,
        command: &str,
        timeout: Option<Duration>,
    ) -> Result<ExecResult, SynapticError> {
        let cmd = if let Some(dur) = timeout {
            format!("timeout {}s sh -c {}", dur.as_secs(), shell_escape(command))
        } else {
            command.to_string()
        };

        self.docker_exec(&cmd).await
    }

    fn supports_execution(&self) -> bool {
        true
    }
}

fn shell_escape(s: &str) -> String {
    format!("'{}'", s.replace('\'', "'\\''"))
}
