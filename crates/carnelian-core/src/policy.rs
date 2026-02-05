//! Policy engine for capability-based security

#[derive(Default)]
pub struct PolicyEngine;

impl PolicyEngine {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}
