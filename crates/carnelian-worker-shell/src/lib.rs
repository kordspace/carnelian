//! Shell Worker Wrapper
//!
//! Manages shell worker processes for executing shell-based skills

pub struct ShellWorker {
    worker_path: String,
}

impl ShellWorker {
    #[must_use]
    pub const fn new(worker_path: String) -> Self {
        Self { worker_path }
    }

    #[must_use]
    pub fn worker_path(&self) -> &str {
        &self.worker_path
    }
}
