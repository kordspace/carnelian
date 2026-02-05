//! Policy engine for capability-based security
//!
//! This module provides database-backed capability checking for the security model.
//! It queries the `capability_grants` table to verify if a subject (identity, skill, etc.)
//! has permission for a specific capability.

use carnelian_common::types::{EventEnvelope, EventLevel, EventType};
use carnelian_common::{Error, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value as JsonValue};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::EventStream;

/// Represents a capability grant from the database
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CapabilityGrant {
    /// Unique identifier for this grant
    pub grant_id: Uuid,
    /// Type of subject: 'identity', 'skill', 'channel', 'session'
    pub subject_type: String,
    /// UUID of the subject
    pub subject_id: Uuid,
    /// Capability key (e.g., 'fs.read', 'net.http')
    pub capability_key: String,
    /// Optional scope restriction (JSON)
    pub scope: Option<JsonValue>,
    /// Optional constraints (JSON)
    pub constraints: Option<JsonValue>,
    /// Identity that approved this grant
    pub approved_by: Option<Uuid>,
    /// When the grant was created
    pub created_at: DateTime<Utc>,
    /// When the grant expires (None = never)
    pub expires_at: Option<DateTime<Utc>>,
}

/// Policy engine for capability-based security
pub struct PolicyEngine {
    pool: PgPool,
}

impl PolicyEngine {
    /// Create a new PolicyEngine with a database connection pool
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Check if a subject has a specific capability
    ///
    /// Returns `Ok(true)` if the subject has a valid (non-expired) grant for the capability,
    /// `Ok(false)` otherwise.
    pub async fn check_capability(
        &self,
        subject_type: &str,
        subject_id: Uuid,
        capability_key: &str,
        event_stream: Option<&EventStream>,
    ) -> Result<bool> {
        let result: Option<Uuid> = sqlx::query_scalar(
            r#"
            SELECT grant_id FROM capability_grants 
            WHERE subject_type = $1 AND subject_id = $2 AND capability_key = $3
              AND (expires_at IS NULL OR expires_at > NOW())
            LIMIT 1
            "#,
        )
        .bind(subject_type)
        .bind(subject_id)
        .bind(capability_key)
        .fetch_optional(&self.pool)
        .await
        .map_err(Error::Database)?;

        let granted = result.is_some();

        tracing::debug!(
            subject_type = %subject_type,
            subject_id = %subject_id,
            capability_key = %capability_key,
            granted = %granted,
            "Capability check"
        );

        // Emit event on denial
        if !granted {
            if let Some(stream) = event_stream {
                stream.publish(EventEnvelope::new(
                    EventLevel::Warn,
                    EventType::CapabilityDenied,
                    json!({
                        "subject_type": subject_type,
                        "subject_id": subject_id,
                        "capability_key": capability_key
                    }),
                ));
            }
        }

        Ok(granted)
    }

    /// Grant a capability to a subject
    ///
    /// Returns the generated `grant_id` UUID on success.
    pub async fn grant_capability(
        &self,
        subject_type: &str,
        subject_id: Uuid,
        capability_key: &str,
        scope: Option<JsonValue>,
        constraints: Option<JsonValue>,
        approved_by: Option<Uuid>,
        expires_at: Option<DateTime<Utc>>,
        event_stream: Option<&EventStream>,
    ) -> Result<Uuid> {
        let grant_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO capability_grants 
              (subject_type, subject_id, capability_key, scope, constraints, approved_by, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING grant_id
            "#,
        )
        .bind(subject_type)
        .bind(subject_id)
        .bind(capability_key)
        .bind(&scope)
        .bind(&constraints)
        .bind(approved_by)
        .bind(expires_at)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| {
            // Check for foreign key constraint violation (capability_key doesn't exist)
            if e.to_string().contains("foreign key") || e.to_string().contains("violates") {
                Error::Security(format!("Invalid capability key: {}", capability_key))
            } else {
                Error::Database(e)
            }
        })?;

        tracing::info!(
            grant_id = %grant_id,
            subject_type = %subject_type,
            subject_id = %subject_id,
            capability_key = %capability_key,
            "Capability granted"
        );

        // Emit grant event
        if let Some(stream) = event_stream {
            stream.publish(EventEnvelope::new(
                EventLevel::Info,
                EventType::CapabilityGranted,
                json!({
                    "grant_id": grant_id,
                    "subject_type": subject_type,
                    "subject_id": subject_id,
                    "capability_key": capability_key
                }),
            ));
        }

        Ok(grant_id)
    }

    /// Revoke a capability grant
    ///
    /// Returns `Ok(true)` if a grant was deleted, `Ok(false)` if the grant_id didn't exist.
    pub async fn revoke_capability(
        &self,
        grant_id: Uuid,
        event_stream: Option<&EventStream>,
    ) -> Result<bool> {
        let result = sqlx::query(r#"DELETE FROM capability_grants WHERE grant_id = $1"#)
            .bind(grant_id)
            .execute(&self.pool)
            .await
            .map_err(Error::Database)?;

        let revoked = result.rows_affected() > 0;

        if revoked {
            tracing::info!(grant_id = %grant_id, "Capability revoked");

            // Emit revocation event
            if let Some(stream) = event_stream {
                stream.publish(EventEnvelope::new(
                    EventLevel::Info,
                    EventType::CapabilityRevoked,
                    json!({ "grant_id": grant_id }),
                ));
            }
        }

        Ok(revoked)
    }

    /// List all valid (non-expired) grants for a subject
    pub async fn list_grants_for_subject(
        &self,
        subject_type: &str,
        subject_id: Uuid,
    ) -> Result<Vec<CapabilityGrant>> {
        let rows: Vec<CapabilityGrant> = sqlx::query_as(
            r#"
            SELECT 
                grant_id,
                subject_type,
                subject_id,
                capability_key,
                scope,
                constraints,
                approved_by,
                created_at,
                expires_at
            FROM capability_grants
            WHERE subject_type = $1 AND subject_id = $2
              AND (expires_at IS NULL OR expires_at > NOW())
            ORDER BY created_at DESC
            "#,
        )
        .bind(subject_type)
        .bind(subject_id)
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(rows)
    }

    /// Check if an identity can execute a task with a specific skill
    ///
    /// This verifies:
    /// 1. The identity has `task.create` capability
    /// 2. The skill has all its required capabilities granted
    ///
    /// TODO: Full integration with task execution in Phase 2
    pub async fn check_task_execution(
        &self,
        identity_id: Uuid,
        skill_id: Uuid,
        event_stream: Option<&EventStream>,
    ) -> Result<()> {
        // Check if identity has task.create capability
        if !self
            .check_capability("identity", identity_id, "task.create", event_stream)
            .await?
        {
            return Err(Error::Security(
                "Identity lacks task.create capability".to_string(),
            ));
        }

        // Get skill's required capabilities
        let required_capabilities: Option<JsonValue> = sqlx::query_scalar(
            r#"SELECT capabilities_required FROM skills WHERE skill_id = $1"#,
        )
        .bind(skill_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(Error::Database)?;

        // Check each required capability for the skill
        if let Some(caps) = required_capabilities {
            if let Some(cap_array) = caps.as_array() {
                for cap in cap_array {
                    if let Some(capability_key) = cap.as_str() {
                        if !self
                            .check_capability("skill", skill_id, capability_key, event_stream)
                            .await?
                        {
                            return Err(Error::Security(format!(
                                "Skill lacks required capability: {}",
                                capability_key
                            )));
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_check_capability_returns_false_for_nonexistent() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect("postgresql://carnelian:carnelian@localhost:5432/carnelian")
            .await
            .expect("Failed to connect to database");

        let engine = PolicyEngine::new(pool);
        let result = engine
            .check_capability("identity", Uuid::new_v4(), "nonexistent.capability", None)
            .await
            .expect("Check should not error");

        assert!(!result, "Should return false for non-existent grant");
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_grant_and_check_capability() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect("postgresql://carnelian:carnelian@localhost:5432/carnelian")
            .await
            .expect("Failed to connect to database");

        let engine = PolicyEngine::new(pool);
        let subject_id = Uuid::new_v4();

        // Initially should not have capability
        let before = engine
            .check_capability("identity", subject_id, "fs.read", None)
            .await
            .expect("Check should not error");
        assert!(!before, "Should not have capability before grant");

        // Grant the capability
        let grant_id = engine
            .grant_capability("identity", subject_id, "fs.read", None, None, None, None, None)
            .await
            .expect("Grant should succeed");

        // Now should have capability
        let after = engine
            .check_capability("identity", subject_id, "fs.read", None)
            .await
            .expect("Check should not error");
        assert!(after, "Should have capability after grant");

        // Revoke and verify
        let revoked = engine
            .revoke_capability(grant_id, None)
            .await
            .expect("Revoke should not error");
        assert!(revoked, "Should have revoked the grant");

        // Should no longer have capability
        let after_revoke = engine
            .check_capability("identity", subject_id, "fs.read", None)
            .await
            .expect("Check should not error");
        assert!(!after_revoke, "Should not have capability after revocation");
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_expired_grants_not_valid() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect("postgresql://carnelian:carnelian@localhost:5432/carnelian")
            .await
            .expect("Failed to connect to database");

        let engine = PolicyEngine::new(pool);
        let subject_id = Uuid::new_v4();

        // Grant with past expiration
        let past = Utc::now() - chrono::Duration::hours(1);
        let _grant_id = engine
            .grant_capability(
                "identity",
                subject_id,
                "fs.read",
                None,
                None,
                None,
                Some(past),
                None,
            )
            .await
            .expect("Grant should succeed");

        // Should not have capability (expired)
        let result = engine
            .check_capability("identity", subject_id, "fs.read", None)
            .await
            .expect("Check should not error");
        assert!(!result, "Expired grant should not be valid");
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_list_grants_for_subject() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect("postgresql://carnelian:carnelian@localhost:5432/carnelian")
            .await
            .expect("Failed to connect to database");

        let engine = PolicyEngine::new(pool);
        let subject_id = Uuid::new_v4();

        // Grant multiple capabilities
        engine
            .grant_capability("identity", subject_id, "fs.read", None, None, None, None, None)
            .await
            .expect("Grant should succeed");
        engine
            .grant_capability("identity", subject_id, "fs.write", None, None, None, None, None)
            .await
            .expect("Grant should succeed");

        // List grants
        let grants = engine
            .list_grants_for_subject("identity", subject_id)
            .await
            .expect("List should not error");

        assert!(grants.len() >= 2, "Should have at least 2 grants");
    }
}
