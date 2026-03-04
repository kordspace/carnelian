//! Sub-agent lifecycle management for Carnelian OS
//!
//! This module provides database-backed sub-agent creation, management, and lifecycle
//! control. Sub-agents are specialized identities with scoped capabilities that can
//! be spawned as worker processes.
//!
//! # Architecture
//!
//! ```text
//! SubAgentManager → identities + sub_agents tables
//!        ↓
//! PolicyEngine (capability validation)
//!        ↓
//! WorkerManager (process spawning with identity pack)
//!        ↓
//! EventStream (lifecycle events)
//! ```
//!
//! # Example
//!
//! ```ignore
//! let manager = SubAgentManager::new(pool, Some(event_stream));
//! let sub_agent = manager.create_sub_agent(
//!     parent_id,
//!     created_by,
//!     CreateSubAgentRequest {
//!         name: "CodeReviewer".to_string(),
//!         role: "code_review".to_string(),
//!         directives: Some(json!(["Review code for security", "Check style"])),
//!         model_provider: Some(provider_id),
//!         ephemeral: false,
//!         capabilities: vec!["fs.read".to_string()],
//!     },
//!     &policy_engine,
//!     Some(&ledger),
//! ).await?;
//! ```

use std::sync::Arc;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use carnelian_common::types::{EventEnvelope, EventLevel, EventType};
use carnelian_common::{Error, Result};

use crate::events::EventStream;
use crate::ledger::Ledger;
use crate::policy::PolicyEngine;

// =============================================================================
// TYPES
// =============================================================================

/// A sub-agent record joining `identities` and `sub_agents` tables.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubAgent {
    pub sub_agent_id: Uuid,
    pub parent_id: Uuid,
    pub created_by: Uuid,
    pub model_provider: Option<Uuid>,
    pub name: String,
    pub role: String,
    pub directives: Option<JsonValue>,
    pub ephemeral: bool,
    pub created_at: DateTime<Utc>,
    pub last_active_at: DateTime<Utc>,
    pub terminated_at: Option<DateTime<Utc>>,
}

/// Request payload for creating a sub-agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateSubAgentRequest {
    pub name: String,
    pub role: String,
    /// Explicit parent identity. If provided, the caller must be this identity
    /// or an allowed ancestor; otherwise the caller identity is used as parent.
    #[serde(default)]
    pub parent_id: Option<Uuid>,
    #[serde(default)]
    pub directives: Option<JsonValue>,
    #[serde(default)]
    pub model_provider: Option<Uuid>,
    #[serde(default)]
    pub ephemeral: bool,
    /// Capabilities to grant to the sub-agent after creation.
    #[serde(default)]
    pub capabilities: Vec<String>,
    /// Worker runtime to spawn for this sub-agent ("node", "python", "shell").
    /// Stored in directives JSONB as `_runtime`. Defaults to "node".
    #[serde(default = "default_runtime")]
    pub runtime: String,
}

fn default_runtime() -> String {
    "node".to_string()
}

/// Request payload for updating a sub-agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateSubAgentRequest {
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub role: Option<String>,
    #[serde(default)]
    pub directives: Option<JsonValue>,
    #[serde(default)]
    pub model_provider: Option<Uuid>,
}

/// Identity pack passed to worker processes for sub-agent context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IdentityPack {
    pub identity_id: Uuid,
    pub name: String,
    pub role: String,
    pub directives: Vec<String>,
    pub capabilities: Vec<String>,
}

// =============================================================================
// SUB-AGENT MANAGER
// =============================================================================

/// Manages sub-agent lifecycle, CRUD operations, and event emission.
///
/// Follows the established manager pattern (see `MemoryManager`, `SessionManager`)
/// with database-backed persistence and optional event stream integration.
pub struct SubAgentManager {
    /// Database connection pool
    pool: PgPool,
    /// Optional event stream for audit trail
    event_stream: Option<Arc<EventStream>>,
}

impl SubAgentManager {
    /// Create a new SubAgentManager.
    ///
    /// # Arguments
    ///
    /// * `pool` - PostgreSQL connection pool
    /// * `event_stream` - Optional event stream for audit events
    pub fn new(pool: PgPool, event_stream: Option<Arc<EventStream>>) -> Self {
        Self { pool, event_stream }
    }

    // =========================================================================
    // CRUD OPERATIONS
    // =========================================================================

    /// Create a new sub-agent.
    ///
    /// Creates an identity entry first, then a sub_agents entry in a transaction.
    /// Validates capabilities via `PolicyEngine::check_capability` on the parent,
    /// then grants requested capabilities to the new sub-agent.
    /// Emits `EventType::SubAgentCreated` on success.
    ///
    /// # Arguments
    ///
    /// * `parent_id` - Identity ID of the parent agent
    /// * `created_by` - Identity ID of the creator
    /// * `request` - Creation parameters
    /// * `policy_engine` - Policy engine for capability validation and granting
    /// * `ledger` - Optional audit ledger for capability grant logging
    pub async fn create_sub_agent(
        &self,
        parent_id: Uuid,
        created_by: Uuid,
        request: CreateSubAgentRequest,
        policy_engine: &PolicyEngine,
        ledger: Option<&Ledger>,
    ) -> Result<SubAgent> {
        // Validate parent has permission to create sub-agents
        let can_create = policy_engine
            .check_capability(
                "identity",
                &parent_id.to_string(),
                "sub_agent.create",
                self.event_stream.as_deref(),
            )
            .await?;

        if !can_create {
            return Err(Error::Security(
                "Parent identity lacks sub_agent.create capability".to_string(),
            ));
        }

        // Merge runtime into directives JSONB so it persists with the record
        let mut merged_directives = request.directives.clone().unwrap_or_else(|| json!({}));
        if let Some(obj) = merged_directives.as_object_mut() {
            obj.insert("_runtime".to_string(), json!(request.runtime));
        }

        // Begin transaction for atomic identity + sub_agent creation
        let mut tx = self.pool.begin().await.map_err(Error::Database)?;

        // 1. Insert into identities table
        let identity_id = sqlx::query_scalar::<_, Uuid>(
            "INSERT INTO identities (name, identity_type, directives) \
             VALUES ($1, 'sub_agent', $2) RETURNING identity_id",
        )
        .bind(&request.name)
        .bind(&merged_directives)
        .fetch_one(&mut *tx)
        .await
        .map_err(Error::Database)?;

        // 2. Insert into sub_agents table
        sqlx::query(
            "INSERT INTO sub_agents (sub_agent_id, parent_id, created_by, name, role, directives, model_provider, ephemeral) \
             VALUES ($1, $2, $3, $4, $5, $6, $7, $8)",
        )
        .bind(identity_id)
        .bind(parent_id)
        .bind(created_by)
        .bind(&request.name)
        .bind(&request.role)
        .bind(&merged_directives)
        .bind(request.model_provider)
        .bind(request.ephemeral)
        .execute(&mut *tx)
        .await
        .map_err(Error::Database)?;

        tx.commit().await.map_err(Error::Database)?;

        // Initialize XP row for the new sub-agent
        if let Err(e) = sqlx::query(
            "INSERT INTO agent_xp (identity_id, total_xp, level, xp_to_next_level) \
             VALUES ($1, 0, 1, (SELECT total_xp_required FROM level_progression WHERE level = 2)) \
             ON CONFLICT (identity_id) DO NOTHING",
        )
        .bind(identity_id)
        .execute(&self.pool)
        .await
        {
            tracing::warn!(
                sub_agent_id = %identity_id,
                error = %e,
                "Failed to initialize agent_xp row for sub-agent"
            );
        }

        // Grant requested capabilities to the new sub-agent
        for capability_key in &request.capabilities {
            if let Err(e) = policy_engine
                .grant_capability(
                    "identity",
                    &identity_id.to_string(),
                    capability_key,
                    None,
                    None,
                    Some(created_by),
                    None,
                    self.event_stream.as_deref(),
                    ledger,
                    None,
                    None,
                )
                .await
            {
                tracing::warn!(
                    sub_agent_id = %identity_id,
                    capability = %capability_key,
                    error = %e,
                    "Failed to grant capability to sub-agent"
                );
            }
        }

        let sub_agent = self
            .get_sub_agent(identity_id)
            .await?
            .ok_or_else(|| Error::Database(sqlx::Error::RowNotFound))?;

        tracing::info!(
            sub_agent_id = %identity_id,
            parent_id = %parent_id,
            name = %request.name,
            role = %request.role,
            "Sub-agent created"
        );

        self.emit_event(
            EventType::SubAgentCreated,
            json!({
                "sub_agent_id": identity_id,
                "name": request.name,
                "role": request.role,
                "parent_id": parent_id,
            }),
            Some(created_by),
        );

        Ok(sub_agent)
    }

    /// Retrieve a sub-agent by ID.
    ///
    /// Joins `identities` and `sub_agents` tables. Returns `None` if not found.
    pub async fn get_sub_agent(&self, sub_agent_id: Uuid) -> Result<Option<SubAgent>> {
        let row = sqlx::query(
            "SELECT sa.sub_agent_id, sa.parent_id, sa.created_by, sa.model_provider, \
                    sa.name, sa.role, sa.directives, sa.ephemeral, \
                    sa.created_at, sa.last_active_at, sa.terminated_at \
             FROM sub_agents sa \
             JOIN identities i ON i.identity_id = sa.sub_agent_id \
             WHERE sa.sub_agent_id = $1",
        )
        .bind(sub_agent_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(row.map(|r| SubAgent {
            sub_agent_id: r.get("sub_agent_id"),
            parent_id: r.get("parent_id"),
            created_by: r.get("created_by"),
            model_provider: r.get("model_provider"),
            name: r.get("name"),
            role: r.get("role"),
            directives: r.get("directives"),
            ephemeral: r.get("ephemeral"),
            created_at: r.get("created_at"),
            last_active_at: r.get("last_active_at"),
            terminated_at: r.get("terminated_at"),
        }))
    }

    /// List sub-agents, optionally filtered by parent and termination status.
    ///
    /// # Arguments
    ///
    /// * `parent_id` - If provided, only return sub-agents with this parent
    /// * `include_terminated` - If false, exclude sub-agents with `terminated_at` set
    pub async fn list_sub_agents(
        &self,
        parent_id: Option<Uuid>,
        include_terminated: bool,
    ) -> Result<Vec<SubAgent>> {
        let mut query = String::from(
            "SELECT sa.sub_agent_id, sa.parent_id, sa.created_by, sa.model_provider, \
                    sa.name, sa.role, sa.directives, sa.ephemeral, \
                    sa.created_at, sa.last_active_at, sa.terminated_at \
             FROM sub_agents sa \
             JOIN identities i ON i.identity_id = sa.sub_agent_id \
             WHERE 1=1",
        );

        if let Some(pid) = parent_id {
            query.push_str(&format!(" AND sa.parent_id = '{}'", pid));
        }
        if !include_terminated {
            query.push_str(" AND sa.terminated_at IS NULL");
        }
        query.push_str(" ORDER BY sa.created_at DESC");

        let rows = sqlx::query(&query)
            .fetch_all(&self.pool)
            .await
            .map_err(Error::Database)?;

        Ok(rows
            .into_iter()
            .map(|r| SubAgent {
                sub_agent_id: r.get("sub_agent_id"),
                parent_id: r.get("parent_id"),
                created_by: r.get("created_by"),
                model_provider: r.get("model_provider"),
                name: r.get("name"),
                role: r.get("role"),
                directives: r.get("directives"),
                ephemeral: r.get("ephemeral"),
                created_at: r.get("created_at"),
                last_active_at: r.get("last_active_at"),
                terminated_at: r.get("terminated_at"),
            })
            .collect())
    }

    /// Update a sub-agent's mutable fields.
    ///
    /// Updates both the `sub_agents` and `identities` tables.
    /// Emits `EventType::SubAgentUpdated` on success.
    pub async fn update_sub_agent(
        &self,
        sub_agent_id: Uuid,
        request: UpdateSubAgentRequest,
    ) -> Result<SubAgent> {
        let mut tx = self.pool.begin().await.map_err(Error::Database)?;

        // Update sub_agents table
        sqlx::query(
            "UPDATE sub_agents SET \
                name = COALESCE($2, name), \
                role = COALESCE($3, role), \
                directives = COALESCE($4, directives), \
                model_provider = COALESCE($5, model_provider) \
             WHERE sub_agent_id = $1",
        )
        .bind(sub_agent_id)
        .bind(&request.name)
        .bind(&request.role)
        .bind(&request.directives)
        .bind(request.model_provider)
        .execute(&mut *tx)
        .await
        .map_err(Error::Database)?;

        // Update identities table name if changed
        if let Some(ref name) = request.name {
            sqlx::query("UPDATE identities SET name = $2 WHERE identity_id = $1")
                .bind(sub_agent_id)
                .bind(name)
                .execute(&mut *tx)
                .await
                .map_err(Error::Database)?;
        }

        // Update identities directives if changed
        if let Some(ref directives) = request.directives {
            sqlx::query("UPDATE identities SET directives = $2 WHERE identity_id = $1")
                .bind(sub_agent_id)
                .bind(directives)
                .execute(&mut *tx)
                .await
                .map_err(Error::Database)?;
        }

        tx.commit().await.map_err(Error::Database)?;

        let sub_agent = self
            .get_sub_agent(sub_agent_id)
            .await?
            .ok_or_else(|| Error::Database(sqlx::Error::RowNotFound))?;

        tracing::info!(sub_agent_id = %sub_agent_id, "Sub-agent updated");

        self.emit_event(
            EventType::SubAgentUpdated,
            json!({
                "sub_agent_id": sub_agent_id,
                "name": sub_agent.name,
                "role": sub_agent.role,
            }),
            None,
        );

        Ok(sub_agent)
    }

    /// Soft-delete a sub-agent by setting `terminated_at`.
    ///
    /// Emits `EventType::SubAgentTerminated` on success.
    /// Returns `true` if a row was updated, `false` if the sub-agent was not found.
    pub async fn delete_sub_agent(&self, sub_agent_id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            "UPDATE sub_agents SET terminated_at = NOW() \
             WHERE sub_agent_id = $1 AND terminated_at IS NULL",
        )
        .bind(sub_agent_id)
        .execute(&self.pool)
        .await
        .map_err(Error::Database)?;

        let terminated = result.rows_affected() > 0;

        if terminated {
            tracing::info!(sub_agent_id = %sub_agent_id, "Sub-agent terminated");

            self.emit_event(
                EventType::SubAgentTerminated,
                json!({ "sub_agent_id": sub_agent_id }),
                None,
            );
        }

        Ok(terminated)
    }

    /// Pause a sub-agent by setting a pause flag in directives JSONB.
    ///
    /// Emits `EventType::SubAgentPaused` on success.
    pub async fn pause_sub_agent(&self, sub_agent_id: Uuid) -> Result<()> {
        let result = sqlx::query(
            "UPDATE sub_agents SET directives = COALESCE(directives, '{}'::jsonb) || '{\"_paused\": true}'::jsonb \
             WHERE sub_agent_id = $1 AND terminated_at IS NULL",
        )
        .bind(sub_agent_id)
        .execute(&self.pool)
        .await
        .map_err(Error::Database)?;

        if result.rows_affected() == 0 {
            return Err(Error::Config(format!(
                "Sub-agent {} not found or already terminated",
                sub_agent_id
            )));
        }

        tracing::info!(sub_agent_id = %sub_agent_id, "Sub-agent paused");

        self.emit_event(
            EventType::SubAgentPaused,
            json!({ "sub_agent_id": sub_agent_id }),
            None,
        );

        Ok(())
    }

    /// Resume a paused sub-agent by removing the pause flag from directives.
    ///
    /// Emits `EventType::SubAgentResumed` on success.
    pub async fn resume_sub_agent(&self, sub_agent_id: Uuid) -> Result<()> {
        let result = sqlx::query(
            "UPDATE sub_agents SET directives = directives - '_paused' \
             WHERE sub_agent_id = $1 AND terminated_at IS NULL",
        )
        .bind(sub_agent_id)
        .execute(&self.pool)
        .await
        .map_err(Error::Database)?;

        if result.rows_affected() == 0 {
            return Err(Error::Config(format!(
                "Sub-agent {} not found or already terminated",
                sub_agent_id
            )));
        }

        tracing::info!(sub_agent_id = %sub_agent_id, "Sub-agent resumed");

        self.emit_event(
            EventType::SubAgentResumed,
            json!({ "sub_agent_id": sub_agent_id }),
            None,
        );

        Ok(())
    }

    /// Update the `last_active_at` timestamp for a sub-agent.
    pub async fn update_last_active(&self, sub_agent_id: Uuid) -> Result<()> {
        sqlx::query("UPDATE sub_agents SET last_active_at = NOW() WHERE sub_agent_id = $1")
            .bind(sub_agent_id)
            .execute(&self.pool)
            .await
            .map_err(Error::Database)?;

        Ok(())
    }

    /// Build an `IdentityPack` for a sub-agent by reading its record and
    /// querying granted capabilities from the database.
    pub async fn build_identity_pack(&self, sub_agent: &SubAgent) -> Result<IdentityPack> {
        // Extract directives as a Vec<String>
        let directives: Vec<String> = sub_agent
            .directives
            .as_ref()
            .and_then(|d| d.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();

        // Query granted capabilities for this identity
        let capabilities: Vec<String> = sqlx::query_scalar(
            "SELECT capability_key FROM capability_grants \
             WHERE subject_type = 'identity' AND subject_id = $1 \
               AND (expires_at IS NULL OR expires_at > NOW())",
        )
        .bind(sub_agent.sub_agent_id.to_string())
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(IdentityPack {
            identity_id: sub_agent.sub_agent_id,
            name: sub_agent.name.clone(),
            role: sub_agent.role.clone(),
            directives,
            capabilities,
        })
    }

    /// Extract the stored runtime string from a sub-agent's directives JSONB.
    /// Falls back to "node" if not set.
    pub fn extract_runtime(sub_agent: &SubAgent) -> String {
        sub_agent
            .directives
            .as_ref()
            .and_then(|d| d.get("_runtime"))
            .and_then(|v| v.as_str())
            .unwrap_or("node")
            .to_string()
    }

    // =========================================================================
    // HELPERS
    // =========================================================================

    /// Emit an event to the event stream if configured.
    fn emit_event(&self, event_type: EventType, payload: JsonValue, actor_id: Option<Uuid>) {
        if let Some(ref stream) = self.event_stream {
            let mut envelope = EventEnvelope::new(EventLevel::Info, event_type, payload);
            if let Some(actor) = actor_id {
                envelope = envelope.with_actor_id(actor.to_string());
            }
            stream.publish(envelope);
        }
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_sub_agent_request_defaults() {
        let json_str = r#"{"name": "TestAgent", "role": "test"}"#;
        let req: CreateSubAgentRequest = serde_json::from_str(json_str).unwrap();
        assert_eq!(req.name, "TestAgent");
        assert_eq!(req.role, "test");
        assert!(req.directives.is_none());
        assert!(req.model_provider.is_none());
        assert!(!req.ephemeral);
        assert!(req.capabilities.is_empty());
    }

    #[test]
    fn test_update_sub_agent_request_partial() {
        let json_str = r#"{"name": "NewName"}"#;
        let req: UpdateSubAgentRequest = serde_json::from_str(json_str).unwrap();
        assert_eq!(req.name, Some("NewName".to_string()));
        assert!(req.role.is_none());
        assert!(req.directives.is_none());
        assert!(req.model_provider.is_none());
    }

    #[test]
    fn test_identity_pack_serialization() {
        let pack = IdentityPack {
            identity_id: Uuid::nil(),
            name: "TestAgent".to_string(),
            role: "code_review".to_string(),
            directives: vec!["Review code".to_string()],
            capabilities: vec!["fs.read".to_string()],
        };
        let json = serde_json::to_string(&pack).unwrap();
        assert!(json.contains("TestAgent"));
        assert!(json.contains("code_review"));
        assert!(json.contains("fs.read"));
    }

    #[test]
    fn test_sub_agent_serialization() {
        let agent = SubAgent {
            sub_agent_id: Uuid::nil(),
            parent_id: Uuid::nil(),
            created_by: Uuid::nil(),
            model_provider: None,
            name: "Test".to_string(),
            role: "test".to_string(),
            directives: Some(json!(["directive1"])),
            ephemeral: false,
            created_at: Utc::now(),
            last_active_at: Utc::now(),
            terminated_at: None,
        };
        let json = serde_json::to_value(&agent).unwrap();
        assert_eq!(json["name"], "Test");
        assert_eq!(json["role"], "test");
        assert!(json["terminated_at"].is_null());
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_create_and_retrieve_sub_agent() {
        unimplemented!("Run with: cargo test --test sub_agent -- --ignored");
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_list_sub_agents_with_filters() {
        unimplemented!("Run with: cargo test --test sub_agent -- --ignored");
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_pause_and_resume_sub_agent() {
        unimplemented!("Run with: cargo test --test sub_agent -- --ignored");
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_soft_delete_sub_agent() {
        unimplemented!("Run with: cargo test --test sub_agent -- --ignored");
    }
}
