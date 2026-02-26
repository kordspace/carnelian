//! Native Worker Wrapper
//!
//! Provides native operations (git, file hash, docker, directory listing)
//! as an in-process worker transport without subprocess overhead.

use std::sync::Arc;

use carnelian_core::worker::{NativeWorkerTransport, WorkerTransport};
use carnelian_core::{Config, EventStream};

/// Wrapper around NativeWorkerTransport for external use.
///
/// This struct provides a standalone, externally-usable interface to the native
/// operations transport defined inline in carnelian-core. It follows the same
/// pattern as PythonWorkerTransport in carnelian-worker-python.
pub struct NativeOpsTransport {
    inner: Arc<NativeWorkerTransport>,
}

impl NativeOpsTransport {
    /// Create a new NativeOpsTransport with the given worker ID and configuration.
    ///
    /// # Arguments
    ///
    /// * `worker_id` - Unique identifier for this worker
    /// * `event_stream` - Event stream for publishing lifecycle events
    /// * `config` - Application configuration
    pub fn new(
        worker_id: String,
        event_stream: Arc<EventStream>,
        config: Arc<Config>,
    ) -> Self {
        let inner = Arc::new(NativeWorkerTransport::new(worker_id, event_stream, config));
        Self { inner }
    }
}

#[async_trait::async_trait]
impl WorkerTransport for NativeOpsTransport {
    async fn invoke(
        &self,
        request: carnelian_common::types::InvokeRequest,
    ) -> anyhow::Result<carnelian_common::types::InvokeResponse> {
        self.inner.invoke(request).await.map_err(|e| e.into())
    }

    async fn stream_events(
        &self,
        run_id: carnelian_common::types::RunId,
    ) -> anyhow::Result<tokio::sync::mpsc::Receiver<carnelian_common::types::StreamEvent>> {
        self.inner.stream_events(run_id).await.map_err(|e| e.into())
    }

    async fn cancel(
        &self,
        run_id: carnelian_common::types::RunId,
        reason: String,
    ) -> anyhow::Result<()> {
        self.inner.cancel(run_id, reason).await.map_err(|e| e.into())
    }

    async fn health(
        &self,
    ) -> anyhow::Result<carnelian_common::types::HealthResponse> {
        self.inner.health().await.map_err(|e| e.into())
    }

    async fn shutdown(&self) -> anyhow::Result<()> {
        self.inner.shutdown().await.map_err(|e| e.into())
    }
}

/// Re-export the standalone op functions for direct use.
///
/// These functions can be used independently of the transport wrapper
/// when only a specific operation is needed.
pub mod ops {
    use anyhow::{Context, Result};
    use serde_json::json;
    use bollard::exec::{CreateExecOptions, StartExecResults};
    use bollard::container::{LogsOptions, StatsOptions};
    use futures_util::StreamExt;
    use sysinfo::{Disks, Networks, ProcessRefreshKind, RefreshKind, System};

    /// Execute git status on the given path.
    ///
    /// Requires `git.read` capability.
    pub async fn git_status(path: &str) -> Result<serde_json::Value> {
        let output = tokio::process::Command::new("git")
            .args(["status", "--porcelain", path])
            .output()
            .await
            .context("Failed to execute git status")?;
        let stdout = String::from_utf8_lossy(&output.stdout);
        Ok(json!({ "output": stdout.to_string() }))
    }

    /// Compute blake3 hash of a file.
    ///
    /// Requires `fs.read` capability.
    pub async fn file_hash(path: &str) -> Result<serde_json::Value> {
        let bytes = tokio::fs::read(path)
            .await
            .with_context(|| format!("Failed to read file: {}", path))?;
        let hash = blake3::hash(&bytes);
        Ok(json!({ "hash": hash.to_hex().to_string() }))
    }

    /// List running Docker containers.
    ///
    /// Requires `docker.read` capability.
    pub async fn docker_ps() -> Result<serde_json::Value> {
        let docker = bollard::Docker::connect_with_local_defaults()
            .context("Failed to connect to Docker")?;
        let containers = docker
            .list_containers::<String>(None)
            .await
            .context("Failed to list containers")?;
        let container_ids: Vec<String> = containers.iter().filter_map(|c| c.id.clone()).collect();
        Ok(json!({ "containers": container_ids }))
    }

    /// List directory entries.
    ///
    /// Requires `fs.read` capability.
    pub async fn dir_list(path: &str, depth: usize) -> Result<serde_json::Value> {
        let mut entries = Vec::new();
        for entry in walkdir::WalkDir::new(path).max_depth(depth) {
            match entry {
                Ok(e) => entries.push(e.path().to_string_lossy().to_string()),
                Err(e) => tracing::warn!(error = %e, "Error reading directory entry"),
            }
        }
        Ok(json!({ "entries": entries }))
    }

    /// Read file contents with optional size limit.
    ///
    /// Requires `fs.read` capability.
    pub async fn file_read(path: &str, max_bytes: usize) -> Result<serde_json::Value> {
        let bytes = tokio::fs::read(path)
            .await
            .with_context(|| format!("Failed to read file: {}", path))?;
        let original_len = bytes.len();
        let truncated = original_len > max_bytes;
        let content_bytes = if truncated { &bytes[..max_bytes] } else { &bytes };
        let content = String::from_utf8_lossy(content_bytes).to_string();
        Ok(json!({
            "content": content,
            "truncated": truncated,
            "size": original_len
        }))
    }

    /// Write content to a file.
    ///
    /// Requires `fs.write` capability and owner approval.
    pub async fn file_write(path: &str, content: &str) -> Result<serde_json::Value> {
        tokio::fs::write(path, content.as_bytes())
            .await
            .with_context(|| format!("Failed to write file: {}", path))?;
        Ok(json!({
            "written": true,
            "path": path
        }))
    }

    /// Search for pattern in files using ripgrep.
    ///
    /// Requires `fs.read` capability.
    pub async fn file_search(pattern: &str, path: &str) -> Result<serde_json::Value> {
        let output = tokio::process::Command::new("rg")
            .args(["--json", pattern, path])
            .output()
            .await
            .context("Failed to execute ripgrep")?;
        
        if !output.status.success() {
            let exit_code = output.status.code();
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "rg search failed with exit code {:?}: {}",
                exit_code, stderr
            );
        }
        
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(json!({ "output": stdout }))
    }

    /// Delete a file.
    ///
    /// Requires `fs.write` capability and owner approval.
    pub async fn file_delete(path: &str) -> Result<serde_json::Value> {
        tokio::fs::remove_file(path)
            .await
            .with_context(|| format!("Failed to delete file: {}", path))?;
        Ok(json!({
            "deleted": true,
            "path": path
        }))
    }

    /// Move/rename a file.
    ///
    /// Requires `fs.write` capability.
    pub async fn file_move(src: &str, dst: &str) -> Result<serde_json::Value> {
        tokio::fs::rename(src, dst)
            .await
            .with_context(|| format!("Failed to move file from {} to {}", src, dst))?;
        Ok(json!({
            "moved": true,
            "src": src,
            "dst": dst
        }))
    }

    /// Execute git diff on the given path.
    ///
    /// Requires `git.read` capability.
    pub async fn git_diff(path: &str, staged: bool) -> Result<serde_json::Value> {
        let mut args = vec!["diff"];
        if staged {
            args.push("--cached");
        }
        args.push(path);
        
        let output = tokio::process::Command::new("git")
            .args(&args)
            .output()
            .await
            .context("Failed to execute git diff")?;
        
        if !output.status.success() {
            let exit_code = output.status.code();
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "git diff failed with exit code {:?}: {}",
                exit_code, stderr
            );
        }
        
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(json!({ "output": stdout }))
    }

    /// Execute git commit with the given message.
    ///
    /// Requires `git.write` capability and owner approval.
    pub async fn git_commit(path: &str, message: &str) -> Result<serde_json::Value> {
        let output = tokio::process::Command::new("git")
            .args(["commit", "-m", message])
            .current_dir(path)
            .output()
            .await
            .context("Failed to execute git commit")?;
        
        if !output.status.success() {
            let exit_code = output.status.code();
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "git commit failed with exit code {:?}: {}",
                exit_code, stderr
            );
        }
        
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(json!({ "output": stdout }))
    }

    /// Execute git log on the given path.
    ///
    /// Requires `git.read` capability.
    pub async fn git_log(path: &str, max_count: u64, oneline: bool) -> Result<serde_json::Value> {
        let mut args = vec!["log", &format!("--max-count={}", max_count)];
        if oneline {
            args.push("--oneline");
        }
        args.push(path);
        
        let output = tokio::process::Command::new("git")
            .args(&args)
            .output()
            .await
            .context("Failed to execute git log")?;
        
        if !output.status.success() {
            let exit_code = output.status.code();
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "git log failed with exit code {:?}: {}",
                exit_code, stderr
            );
        }
        
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(json!({ "output": stdout }))
    }

    /// Execute git branch to list branches.
    ///
    /// Requires `git.read` capability.
    pub async fn git_branch(path: &str) -> Result<serde_json::Value> {
        let output = tokio::process::Command::new("git")
            .args(["branch", "--list"])
            .current_dir(path)
            .output()
            .await
            .context("Failed to execute git branch")?;
        
        if !output.status.success() {
            let exit_code = output.status.code();
            let stderr = String::from_utf8_lossy(&output.stderr);
            anyhow::bail!(
                "git branch failed with exit code {:?}: {}",
                exit_code, stderr
            );
        }
        
        let stdout = String::from_utf8_lossy(&output.stdout).to_string();
        Ok(json!({ "output": stdout }))
    }

    /// Execute a command in a Docker container.
    ///
    /// Requires `docker.exec` capability and owner approval.
    pub async fn docker_exec(container_id: &str, cmd: Vec<String>) -> Result<serde_json::Value> {
        let docker = bollard::Docker::connect_with_local_defaults()
            .context("Failed to connect to Docker")?;
        
        let exec = docker.create_exec(
            container_id,
            CreateExecOptions {
                cmd: Some(cmd),
                attach_stdout: Some(true),
                attach_stderr: Some(true),
                ..Default::default()
            }
        )
        .await
        .context("Failed to create exec")?;
        
        let exec_id = exec.id;
        match docker.start_exec(&exec_id, None).await {
            Ok(StartExecResults::Attached { mut output, .. }) => {
                let mut collected = String::new();
                while let Some(chunk) = output.next().await {
                    match chunk {
                        Ok(msg) => collected.push_str(&msg.to_string()),
                        Err(e) => anyhow::bail!("Stream error: {}", e),
                    }
                }
                Ok(json!({ "output": collected }))
            }
            Ok(_) => anyhow::bail!("Unexpected exec result"),
            Err(e) => Err(e.into()),
        }
    }

    /// Get logs from a Docker container.
    ///
    /// Requires `docker.read` capability.
    pub async fn docker_logs(container_id: &str, tail: &str) -> Result<serde_json::Value> {
        let docker = bollard::Docker::connect_with_local_defaults()
            .context("Failed to connect to Docker")?;
        
        let mut logs_stream = docker.logs(
            container_id,
            Some(LogsOptions {
                stdout: true,
                stderr: true,
                tail: tail.to_string(),
                ..Default::default()
            })
        );
        
        let mut collected = String::new();
        while let Some(chunk) = logs_stream.next().await {
            match chunk {
                Ok(msg) => collected.push_str(&msg.to_string()),
                Err(e) => anyhow::bail!("Stream error: {}", e),
            }
        }
        
        Ok(json!({ "output": collected }))
    }

    /// Get stats from a Docker container.
    ///
    /// Requires `docker.read` capability.
    pub async fn docker_stats(container_id: &str) -> Result<serde_json::Value> {
        let docker = bollard::Docker::connect_with_local_defaults()
            .context("Failed to connect to Docker")?;
        
        let mut stats_stream = docker.stats(
            container_id,
            Some(StatsOptions {
                stream: false,
                ..Default::default()
            })
        );
        
        match stats_stream.next().await {
            Some(Ok(stats)) => {
                let stats_value = serde_json::to_value(&stats)
                    .context("Failed to serialize stats")?;
                Ok(json!({ "stats": stats_value }))
            }
            Some(Err(e)) => Err(e.into()),
            None => anyhow::bail!("No stats available"),
        }
    }

    /// List running processes.
    ///
    /// Requires `system.read` capability.
    pub async fn process_list() -> Result<serde_json::Value> {
        let sys = System::new_with_specifics(
            RefreshKind::new().with_processes(ProcessRefreshKind::everything())
        );
        
        let processes: Vec<serde_json::Value> = sys.processes()
            .iter()
            .map(|(pid, process)| {
                json!({
                    "pid": pid.as_u32(),
                    "name": process.name(),
                    "cpu_usage": process.cpu_usage(),
                    "memory": process.memory()
                })
            })
            .collect();
        
        Ok(json!({ "processes": processes }))
    }

    /// Get disk usage information.
    ///
    /// Requires `system.read` capability.
    pub async fn disk_usage() -> Result<serde_json::Value> {
        let disks = Disks::new_with_refreshed_list();
        
        let disks_vec: Vec<serde_json::Value> = disks.iter()
            .map(|disk| {
                json!({
                    "name": disk.name().to_string_lossy(),
                    "mount_point": disk.mount_point().to_string_lossy(),
                    "total_space": disk.total_space(),
                    "available_space": disk.available_space(),
                    "file_system": String::from_utf8_lossy(disk.file_system()).to_string()
                })
            })
            .collect();
        
        Ok(json!({ "disks": disks_vec }))
    }

    /// Get network interface statistics.
    ///
    /// Requires `system.read` capability.
    pub async fn network_stats() -> Result<serde_json::Value> {
        let networks = Networks::new_with_refreshed_list();
        
        let networks_vec: Vec<serde_json::Value> = networks.iter()
            .map(|(interface, data)| {
                json!({
                    "interface": interface,
                    "received": data.received(),
                    "transmitted": data.transmitted()
                })
            })
            .collect();
        
        Ok(json!({ "networks": networks_vec }))
    }

    /// Get an environment variable value.
    ///
    /// Requires `env.read` capability. Only allows reading from a predefined allowlist.
    pub async fn env_get(key: &str) -> Result<serde_json::Value> {
        const ALLOWLIST: &[&str] = &[
            "PATH", "HOME", "USER", "USERNAME", "SHELL", "LANG", "LC_ALL",
            "PWD", "TERM", "HOSTNAME", "TMPDIR", "TEMP", "TMP"
        ];
        
        if !ALLOWLIST.contains(&key) {
            anyhow::bail!("env var '{}' is not in the allowed list", key);
        }
        
        let value = std::env::var(key)
            .with_context(|| format!("Failed to get env var: {}", key))?;
        
        Ok(json!({
            "key": key,
            "value": value
        }))
    }
}
