//! Python Worker Wrapper
//!
//! Manages Python worker processes for executing Python-based skills

use std::path::PathBuf;
use std::process::Stdio;
use std::sync::Arc;

use anyhow::{Context, Result};
use carnelian_common::types::{HealthResponse, InvokeRequest, InvokeResponse, RunId};
use carnelian_core::worker::{ProcessJsonlTransport, WorkerTransport};
use carnelian_core::{Config, EventStream};
use tokio::process::{ChildStderr, Command};

/// Detect the Python binary path.
///
/// Tries `python3` first, then falls back to `python`.
///
/// # Errors
///
/// Returns an error if neither is found.
pub fn detect_python_binary() -> Result<PathBuf> {
    which::which("python3")
        .or_else(|_| which::which("python"))
        .context("Failed to find Python binary (tried 'python3', then 'python')")
}

/// Pre-install Python worker requirements.txt if present.
///
/// Runs `pip install -r requirements.txt` quietly.
/// Logs success or failure via tracing.
///
/// # Errors
///
/// Returns an error if pip install fails.
///
/// # Panics
///
/// Panics if the requirements.txt path cannot be converted to a string.
pub async fn install_worker_requirements(python_binary: &PathBuf) -> Result<()> {
    let req_path = PathBuf::from("workers/python-worker/requirements.txt");

    if !req_path.exists() {
        tracing::info!("No requirements.txt found at {}", req_path.display());
        return Ok(());
    }

    tracing::info!(
        "Installing Python worker requirements from {}",
        req_path.display()
    );

    let status = Command::new(python_binary)
        .args([
            "-m",
            "pip",
            "install",
            "-r",
            req_path.to_str().unwrap(),
            "--quiet",
            "--disable-pip-version-check",
        ])
        .status()
        .await
        .context("Failed to spawn pip install command")?;

    if status.success() {
        tracing::info!("Python worker requirements installed successfully");
        Ok(())
    } else {
        let code = status.code().unwrap_or(-1);
        tracing::warn!("pip install failed with exit code: {}", code);
        Err(anyhow::anyhow!("pip install failed with exit code: {code}"))
    }
}

/// Newtype wrapper around `ProcessJsonlTransport` for Python workers.
///
/// Provides Python-specific concerns like binary detection and requirements pre-install.
pub struct PythonWorkerTransport {
    inner: Arc<ProcessJsonlTransport>,
}

impl PythonWorkerTransport {
    /// Spawn a new Python worker process and wrap it in a transport.
    ///
    /// Detects Python binary, pre-installs requirements, and spawns the worker.
    /// Returns the transport and an optional stderr handle.
    ///
    /// # Errors
    ///
    /// Returns an error if Python binary detection, requirements installation, or worker spawn fails.
    pub async fn spawn(
        worker_id: String,
        config: Arc<Config>,
        event_stream: Arc<EventStream>,
    ) -> Result<(Self, Option<ChildStderr>)> {
        // Step 1: Detect Python binary
        let python_bin = detect_python_binary()?;
        tracing::info!("Using Python binary: {}", python_bin.display());

        // Step 2: Pre-install requirements
        install_worker_requirements(&python_bin).await?;

        // Step 3: Build spawn command
        let worker_script = "workers/python-worker/worker.py";
        let mut cmd = Command::new(&python_bin);
        cmd.arg(worker_script)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env("WORKER_ID", &worker_id)
            .env(
                "CARNELIAN_API_URL",
                format!("http://localhost:{}", config.http_port),
            )
            .env(
                "RUST_LOG",
                std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string()),
            );

        // Step 4: Spawn the child process
        let child = cmd
            .spawn()
            .context("Failed to spawn Python worker process")?;

        // Step 5: Create ProcessJsonlTransport and wrap it
        let (transport, stderr) =
            ProcessJsonlTransport::new(worker_id, child, config, event_stream)?;
        let wrapper = Self {
            inner: Arc::new(transport),
        };

        Ok((wrapper, stderr))
    }
}

#[async_trait::async_trait]
impl WorkerTransport for PythonWorkerTransport {
    async fn invoke(&self, request: InvokeRequest) -> carnelian_core::Result<InvokeResponse> {
        self.inner.invoke(request).await
    }

    async fn stream_events(
        &self,
        run_id: RunId,
    ) -> carnelian_core::Result<tokio::sync::mpsc::Receiver<carnelian_common::types::StreamEvent>>
    {
        self.inner.stream_events(run_id).await
    }

    async fn cancel(&self, run_id: RunId, reason: String) -> carnelian_core::Result<()> {
        self.inner.cancel(run_id, reason).await
    }

    async fn health(&self) -> carnelian_core::Result<HealthResponse> {
        self.inner.health().await
    }

    async fn shutdown(&self) -> carnelian_core::Result<()> {
        self.inner.shutdown().await
    }
}
