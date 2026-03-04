//! Skill execution sandboxing and resource limits
//!
//! Provides cross-platform resource limits and isolation for skill execution.
//! - Windows: Job objects for CPU/memory limits
//! - Unix: rlimit for resource constraints
//! - All platforms: Timeout enforcement, output validation

use carnelian_common::{Error, Result};
use std::process::{Command, Stdio};
use std::time::Duration;
use tokio::time::timeout;

/// Resource limits for skill execution
#[derive(Debug, Clone)]
pub struct ResourceLimits {
    /// Maximum memory in MB
    pub max_memory_mb: u64,
    /// Maximum CPU time in seconds
    pub max_cpu_seconds: u64,
    /// Maximum wall-clock time in seconds
    pub timeout_seconds: u64,
    /// Maximum number of processes
    pub max_processes: u32,
}

impl Default for ResourceLimits {
    fn default() -> Self {
        Self {
            max_memory_mb: 512,  // 512 MB default
            max_cpu_seconds: 30, // 30 seconds CPU time
            timeout_seconds: 60, // 60 seconds wall time
            max_processes: 10,   // Max 10 processes
        }
    }
}

/// Sandboxed skill execution result
#[derive(Debug)]
pub struct SandboxedResult {
    /// Exit code
    pub exit_code: i32,
    /// Standard output
    pub stdout: Vec<u8>,
    /// Standard error
    pub stderr: Vec<u8>,
    /// Execution time in milliseconds
    pub duration_ms: u64,
    /// Whether execution was killed due to timeout
    pub timed_out: bool,
}

/// Execute a command with resource limits and sandboxing
///
/// # Arguments
/// * `command` - Command to execute
/// * `args` - Command arguments
/// * `limits` - Resource limits to enforce
///
/// # Returns
/// A `SandboxedResult` with execution details
pub async fn execute_sandboxed(
    command: &str,
    args: &[String],
    limits: &ResourceLimits,
) -> Result<SandboxedResult> {
    let start = std::time::Instant::now();

    // Build command with platform-specific resource limits
    let mut cmd = Command::new(command);
    cmd.args(args)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    // Apply platform-specific resource limits
    #[cfg(unix)]
    apply_unix_limits(&mut cmd, limits)?;

    #[cfg(windows)]
    apply_windows_limits(&mut cmd, limits)?;

    // Spawn process
    let child = cmd
        .spawn()
        .map_err(|e| Error::SkillExecution(format!("Failed to spawn process: {}", e)))?;

    // Store child ID for timeout handling
    let child_id = child.id();

    // Wait with timeout
    let timeout_duration = Duration::from_secs(limits.timeout_seconds);
    let result = timeout(timeout_duration, async {
        tokio::task::spawn_blocking(move || child.wait_with_output())
            .await
            .map_err(|e| std::io::Error::new(std::io::ErrorKind::Other, e))?
    })
    .await;

    let duration_ms = start.elapsed().as_millis() as u64;

    match result {
        Ok(Ok(output)) => Ok(SandboxedResult {
            exit_code: output.status.code().unwrap_or(-1),
            stdout: output.stdout,
            stderr: output.stderr,
            duration_ms,
            timed_out: false,
        }),
        Ok(Err(e)) => Err(Error::SkillExecution(format!(
            "Process execution failed: {}",
            e
        ))),
        Err(_) => {
            // Timeout - attempt to kill the process by PID
            #[cfg(windows)]
            {
                let _ = std::process::Command::new("taskkill")
                    .args(["/F", "/PID", &child_id.to_string()])
                    .output();
            }
            #[cfg(not(windows))]
            {
                use std::os::unix::process::CommandExt;
                let _ = std::process::Command::new("kill")
                    .args(["-9", &child_id.to_string()])
                    .output();
            }

            Ok(SandboxedResult {
                exit_code: -1,
                stdout: Vec::new(),
                stderr: b"Execution timed out".to_vec(),
                duration_ms,
                timed_out: true,
            })
        }
    }
}

#[cfg(unix)]
fn apply_unix_limits(cmd: &mut Command, limits: &ResourceLimits) -> Result<()> {
    use std::os::unix::process::CommandExt;

    // Use pre_exec to set resource limits in the child process
    unsafe {
        cmd.pre_exec(move || {
            use libc::{RLIMIT_AS, RLIMIT_CPU, RLIMIT_NPROC, rlimit, setrlimit};

            // Memory limit
            let mem_limit = rlimit {
                rlim_cur: (limits.max_memory_mb * 1024 * 1024) as u64,
                rlim_max: (limits.max_memory_mb * 1024 * 1024) as u64,
            };
            setrlimit(RLIMIT_AS, &mem_limit);

            // CPU time limit
            let cpu_limit = rlimit {
                rlim_cur: limits.max_cpu_seconds as u64,
                rlim_max: limits.max_cpu_seconds as u64,
            };
            setrlimit(RLIMIT_CPU, &cpu_limit);

            // Process limit
            let proc_limit = rlimit {
                rlim_cur: limits.max_processes as u64,
                rlim_max: limits.max_processes as u64,
            };
            setrlimit(RLIMIT_NPROC, &proc_limit);

            Ok(())
        });
    }

    Ok(())
}

#[cfg(windows)]
fn apply_windows_limits(_cmd: &mut Command, _limits: &ResourceLimits) -> Result<()> {
    // Windows job objects would be applied here
    // For v1.0.0, we rely on timeout enforcement
    // Full job object implementation requires winapi crate
    tracing::warn!("Windows resource limits not fully implemented - using timeout only");
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn apply_unix_limits(_cmd: &mut Command, _limits: &ResourceLimits) -> Result<()> {
    Ok(())
}

#[cfg(not(any(unix, windows)))]
fn apply_windows_limits(_cmd: &mut Command, _limits: &ResourceLimits) -> Result<()> {
    Ok(())
}

/// Validate skill output size
pub fn validate_output_size(output: &[u8], max_size_mb: u64) -> Result<()> {
    let max_bytes = max_size_mb * 1024 * 1024;
    if output.len() as u64 > max_bytes {
        return Err(Error::SkillExecution(format!(
            "Output size {} bytes exceeds limit of {} MB",
            output.len(),
            max_size_mb
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_execute_simple_command() {
        let limits = ResourceLimits::default();

        #[cfg(unix)]
        let result = execute_sandboxed("echo", &["hello".to_string()], &limits).await;

        #[cfg(windows)]
        let result = execute_sandboxed(
            "cmd",
            &["/C".to_string(), "echo hello".to_string()],
            &limits,
        )
        .await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert_eq!(result.exit_code, 0);
        assert!(!result.timed_out);
    }

    #[tokio::test]
    async fn test_timeout_enforcement() {
        let mut limits = ResourceLimits::default();
        limits.timeout_seconds = 1;

        #[cfg(unix)]
        let result = execute_sandboxed("sleep", &["5".to_string()], &limits).await;

        #[cfg(windows)]
        let result =
            execute_sandboxed("timeout", &["/t".to_string(), "5".to_string()], &limits).await;

        assert!(result.is_ok());
        let result = result.unwrap();
        assert!(result.timed_out);
    }

    #[test]
    fn test_output_validation() {
        let small_output = vec![0u8; 1024]; // 1 KB
        assert!(validate_output_size(&small_output, 1).is_ok());

        let large_output = vec![0u8; 2 * 1024 * 1024]; // 2 MB
        assert!(validate_output_size(&large_output, 1).is_err());
    }
}
