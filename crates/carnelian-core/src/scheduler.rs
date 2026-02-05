//! Task scheduler and heartbeat runner

#[derive(Default)]
pub struct Scheduler;

impl Scheduler {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}
