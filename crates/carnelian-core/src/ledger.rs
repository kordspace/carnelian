//! Tamper-resistant ledger for audit trail

#[derive(Default)]
pub struct Ledger;

impl Ledger {
    #[must_use]
    pub const fn new() -> Self {
        Self
    }
}
