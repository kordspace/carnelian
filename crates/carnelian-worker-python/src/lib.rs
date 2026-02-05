//! Python Worker Wrapper
//!
//! Manages Python worker processes for executing Python-based skills

pub struct PythonWorker {
    worker_path: String,
}

impl PythonWorker {
    #[must_use]
    pub const fn new(worker_path: String) -> Self {
        Self { worker_path }
    }

    #[must_use]
    pub fn worker_path(&self) -> &str {
        &self.worker_path
    }
}
