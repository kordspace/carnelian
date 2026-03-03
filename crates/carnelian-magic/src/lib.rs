#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::cast_precision_loss)]
#![allow(clippy::float_cmp)]
#![allow(clippy::ignored_unit_patterns)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::missing_errors_doc)]
#![allow(clippy::missing_panics_doc)]
#![allow(clippy::missing_const_for_fn)]
#![allow(clippy::too_many_arguments)]
#![allow(clippy::doc_markdown)]
#![allow(clippy::module_name_repetitions)]
#![allow(clippy::uninlined_format_args)]
#![allow(clippy::similar_names)]
#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::fn_params_excessive_bools)]
#![allow(clippy::redundant_closure)]
#![allow(clippy::redundant_closure_for_method_calls)]
#![allow(clippy::clone_on_copy)]
#![allow(clippy::must_use_candidate)]
#![allow(clippy::redundant_clone)]
#![allow(clippy::field_reassign_with_default)]
#![allow(clippy::single_match_else)]
#![allow(clippy::if_not_else)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::trivially_copy_pass_by_ref)]
#![allow(clippy::unreadable_literal)]
#![allow(clippy::unnecessary_wraps)]
#![allow(clippy::cognitive_complexity)]
#![allow(clippy::manual_let_else)]

//! Carnelian Magic - Quantum entropy and MantraTree integration
//!
//! This crate provides:
//! - Entropy providers (OS, Quantum Origin, Quantinuum H2, Qiskit)
//! - Mixed entropy with quantum-first fallback
//! - MantraTree placeholder for future integration

pub mod entropy;
pub mod entropy_arc_impl;
mod error;
pub mod hasher;
pub mod mantra;
pub mod verifier;

pub use entropy::{
    EntropyHealth, EntropyProvider, MixedEntropyProvider,
    OsEntropyProvider, QuantumOriginProvider,
    QuantinuumH2Provider, QiskitProvider, SkillBridge,
};
pub use error::{MagicError, Result};
pub use hasher::QuantumHasher;
pub use mantra::{
    MantraCategory,
    MantraContext,
    MantraEntry,
    MantraSelection,
    MantraTree,
};
pub use verifier::{
    QuantumIntegrityVerifier,
    TamperedRow,
    VerificationReport,
    VerificationStatus,
};
