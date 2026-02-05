//! Node.js Worker Wrapper
//!
//! Manages Node.js worker processes for executing 600+ existing skills

pub struct NodeWorker {
    worker_path: String,
}

impl NodeWorker {
    #[must_use]
    pub const fn new(worker_path: String) -> Self {
        Self { worker_path }
    }

    #[must_use]
    pub fn worker_path(&self) -> &str {
        &self.worker_path
    }
}
