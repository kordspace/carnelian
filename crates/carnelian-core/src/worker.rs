//! Worker management and coordination

#[derive(Default)]
pub struct WorkerManager;

impl WorkerManager {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}
