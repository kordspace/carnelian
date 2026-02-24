//! Skill Trait Definition
//!
//! This module defines the core Skill trait for native Rust skill implementations.
//! Skills can be implemented as:
//! - Native Rust dylibs (loaded at runtime)
//! - WASM modules (sandboxed execution)
//! - TypeScript/Node.js workers (existing, process-based)

use crate::skills::SkillManifest;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Unique identifier for a skill
pub type SkillId = String;

/// Input to a skill invocation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillInput {
    /// Action to perform (e.g., "read", "write", "list")
    pub action: String,

    /// Action parameters
    #[serde(default)]
    pub params: serde_json::Value,

    /// Requestor identity
    pub identity_id: Option<uuid::Uuid>,

    /// Session correlation ID
    pub correlation_id: Option<uuid::Uuid>,
}

/// Output from a skill invocation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillOutput {
    /// Success status
    pub success: bool,

    /// Output data
    #[serde(default)]
    pub data: serde_json::Value,

    /// Error message if failed
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,

    /// Execution metadata
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

/// Health status of a skill
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum HealthStatus {
    Healthy,
    Degraded { reason: String },
    Unhealthy { reason: String },
}

/// Execution context provided to skills
#[derive(Debug)]
pub struct SkillContext {
    /// Database connection pool
    pub pool: sqlx::PgPool,

    /// Event stream for logging
    pub event_stream: crate::events::EventStream,

    /// Capability grants for this execution
    pub capabilities: Vec<String>,

    /// Identity of the requestor
    pub identity_id: Option<uuid::Uuid>,

    /// Session correlation ID
    pub correlation_id: Option<uuid::Uuid>,

    /// Task ID if part of a task
    pub task_id: Option<uuid::Uuid>,

    /// Run ID if part of a task run
    pub run_id: Option<uuid::Uuid>,
}

/// Core Skill trait for native Rust implementations
///
/// Implement this trait for skills that will be loaded as native dylibs
/// or compiled directly into the orchestrator.
#[async_trait::async_trait]
pub trait Skill: Send + Sync {
    /// Get the skill manifest
    fn manifest(&self) -> &SkillManifest;

    /// Check if skill has required capabilities
    fn has_capabilities(&self, required: &[String]) -> bool {
        let available = self.manifest().capabilities_required.clone();
        required.iter().all(|cap| available.contains(cap))
    }

    /// Invoke the skill with given input
    ///
    /// # Arguments
    /// * `ctx` - Execution context with database access, event stream, etc.
    /// * `input` - Skill input with action and parameters
    ///
    /// # Returns
    /// Skill output with success status and data
    async fn invoke(
        &self,
        ctx: &SkillContext,
        input: SkillInput,
    ) -> carnelian_common::Result<SkillOutput>;

    /// Health check for the skill
    ///
    /// Default implementation returns Healthy
    async fn health(&self) -> HealthStatus {
        HealthStatus::Healthy
    }

    /// Initialize the skill
    ///
    /// Called once when skill is loaded. Use for setup like:
    /// - Loading configuration
    /// - Setting up caches
    /// - Establishing connections
    async fn init(&mut self) -> carnelian_common::Result<()> {
        Ok(())
    }

    /// Shutdown the skill
    ///
    /// Called when skill is being unloaded. Use for cleanup.
    async fn shutdown(&mut self) -> carnelian_common::Result<()> {
        Ok(())
    }
}

/// Type alias for boxed Skill trait objects
pub type BoxedSkill = Box<dyn Skill>;

/// Factory function type for creating skills
///
/// Used by NativeSkillLoader to instantiate skills from dylibs
pub type SkillFactory = fn() -> BoxedSkill;

/// FFI-safe skill metadata for dylib exports
#[repr(C)]
pub struct SkillMetadata {
    pub name: *const u8,
    pub name_len: usize,
    pub version: *const u8,
    pub version_len: usize,
}

/// FFI export for skill discovery
///
/// Native skills must export this symbol for the loader to find them
#[macro_export]
macro_rules! export_skill {
    ($skill_type:ty) => {
        #[no_mangle]
        pub extern "C" fn _carnelian_skill_create() -> *mut dyn $crate::skills::skill_trait::Skill {
            let skill = Box::new(<$skill_type>::new());
            Box::into_raw(skill)
        }

        #[no_mangle]
        pub extern "C" fn _carnelian_skill_destroy(
            skill: *mut dyn $crate::skills::skill_trait::Skill,
        ) {
            if !skill.is_null() {
                unsafe {
                    Box::from_raw(skill);
                }
            }
        }
    };
}
