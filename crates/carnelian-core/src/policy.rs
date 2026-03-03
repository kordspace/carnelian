//! Policy engine for capability-based security
//!
//! This module provides database-backed capability checking for the security model.
//! It queries the `capability_grants` table to verify if a subject (identity, skill, etc.)
//! has permission for a specific capability.

use carnelian_common::types::{EventEnvelope, EventLevel, EventType};
use carnelian_common::{Error, Result};
use chrono::{DateTime, Utc};
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::EventStream;
use crate::approvals::ApprovalQueue;
use crate::ledger::Ledger;

/// Represents a capability grant from the database
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CapabilityGrant {
    /// Unique identifier for this grant
    pub grant_id: Uuid,
    /// Type of subject: 'identity', 'skill', 'channel', 'session'
    pub subject_type: String,
    /// Subject identifier (UUID string or external reference like "telegram:12345")
    pub subject_id: String,
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

/// Represents a revoked capability grant for cross-instance sync
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RevokedGrantInfo {
    /// Grant ID that was revoked
    pub grant_id: Uuid,
    /// When the grant was revoked
    pub revoked_at: DateTime<Utc>,
    /// Identity that revoked the grant
    pub revoked_by: Option<Uuid>,
    /// Reason for revocation
    pub reason: Option<String>,
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
        subject_id: &str,
        capability_key: &str,
        event_stream: Option<&EventStream>,
    ) -> Result<bool> {
        let result = sqlx::query_scalar::<_, Uuid>(
            r"
            SELECT grant_id FROM capability_grants 
            WHERE subject_type = $1 AND subject_id = $2 AND capability_key = $3
              AND (expires_at IS NULL OR expires_at > NOW())
            LIMIT 1
            ",
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

    /// Grant a capability to a subject.
    ///
    /// If an `approval_queue` is provided, the action is queued for approval
    /// instead of being executed immediately. Returns `Error::ApprovalRequired`
    /// with the approval ID so the caller can track the request.
    ///
    /// Returns the generated `grant_id` UUID on success.
    pub async fn grant_capability(
        &self,
        subject_type: &str,
        subject_id: &str,
        capability_key: &str,
        scope: Option<JsonValue>,
        constraints: Option<JsonValue>,
        approved_by: Option<Uuid>,
        expires_at: Option<DateTime<Utc>>,
        event_stream: Option<&EventStream>,
        ledger: Option<&Ledger>,
        owner_signing_key: Option<&SigningKey>,
        approval_queue: Option<&ApprovalQueue>,
    ) -> Result<Uuid> {
        // If approval queue is provided, queue instead of executing
        if let Some(queue) = approval_queue {
            let payload = json!({
                "subject_type": subject_type,
                "subject_id": subject_id,
                "capability_key": capability_key,
                "scope": scope,
                "constraints": constraints,
                "approved_by": approved_by,
                "expires_at": expires_at,
            });
            let correlation_id = Some(Uuid::now_v7());
            let approval_id = queue
                .queue_action("capability.grant", payload, approved_by, correlation_id)
                .await?;
            return Err(Error::ApprovalRequired(approval_id));
        }

        let grant_id: Uuid = sqlx::query_scalar::<_, Uuid>(
            r"
            INSERT INTO capability_grants 
              (subject_type, subject_id, capability_key, scope, constraints, approved_by, expires_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7)
            RETURNING grant_id
            ",
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

        // Log to audit ledger
        if let Some(ledger) = ledger {
            if let Err(e) = ledger
                .log_capability_grant(
                    grant_id,
                    subject_type,
                    subject_id,
                    capability_key,
                    approved_by,
                    owner_signing_key,
                )
                .await
            {
                tracing::warn!(error = %e, "Failed to log capability grant to ledger");
            }
        }

        Ok(grant_id)
    }

    /// Revoke a capability grant.
    ///
    /// If an `approval_queue` is provided, the action is queued for approval
    /// instead of being executed immediately. Returns `Error::ApprovalRequired`
    /// with the approval ID.
    ///
    /// Returns `Ok(true)` if a grant was deleted, `Ok(false)` if the grant_id didn't exist.
    pub async fn revoke_capability(
        &self,
        grant_id: Uuid,
        revoked_by: Option<Uuid>,
        event_stream: Option<&EventStream>,
        ledger: Option<&Ledger>,
        owner_signing_key: Option<&SigningKey>,
        approval_queue: Option<&ApprovalQueue>,
    ) -> Result<bool> {
        // If approval queue is provided, queue instead of executing
        if let Some(queue) = approval_queue {
            let payload = json!({
                "grant_id": grant_id,
                "revoked_by": revoked_by,
            });
            let correlation_id = Some(Uuid::now_v7());
            let approval_id = queue
                .queue_action("capability.revoke", payload, revoked_by, correlation_id)
                .await?;
            return Err(Error::ApprovalRequired(approval_id));
        }

        let result = sqlx::query!(
            r#"DELETE FROM capability_grants WHERE grant_id = $1"#,
            grant_id,
        )
        .execute(&self.pool)
        .await
        .map_err(Error::Database)?;

        let revoked = result.rows_affected() > 0;

        if revoked {
            tracing::info!(grant_id = %grant_id, "Capability revoked");

            // Insert into revoked_capability_grants for cross-instance sync
            if let Err(e) = sqlx::query!(
                r#"INSERT INTO revoked_capability_grants (grant_id, revoked_by, reason)
                    VALUES ($1, $2, $3)
                    ON CONFLICT (grant_id) DO NOTHING"#,
                grant_id,
                revoked_by,
                Some("explicit_revocation"),
            )
            .execute(&self.pool)
            .await
            {
                tracing::warn!(error = %e, grant_id = %grant_id, "Failed to record grant revocation");
            }

            // Emit revocation event
            if let Some(stream) = event_stream {
                stream.publish(EventEnvelope::new(
                    EventLevel::Info,
                    EventType::CapabilityRevoked,
                    json!({ "grant_id": grant_id }),
                ));
            }
        }

        // Log to audit ledger
        if revoked {
            if let Some(ledger) = ledger {
                if let Err(e) = ledger
                    .log_capability_revoke(grant_id, revoked_by, owner_signing_key)
                    .await
                {
                    tracing::warn!(error = %e, "Failed to log capability revoke to ledger");
                }
            }
        }

        Ok(revoked)
    }

    /// Check if a grant has been revoked (for cross-instance revocation checking)
    pub async fn is_grant_revoked(&self, grant_id: Uuid) -> Result<bool> {
        let exists: Option<(bool,)> = sqlx::query_as(
            "SELECT EXISTS(SELECT 1 FROM revoked_capability_grants WHERE grant_id = $1)",
        )
        .bind(grant_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(exists.map(|(e,)| e).unwrap_or(false))
    }

    /// List revoked grants since a given timestamp (for cross-instance sync)
    pub async fn list_revoked_since(&self, since: DateTime<Utc>) -> Result<Vec<RevokedGrantInfo>> {
        let rows = sqlx::query_as::<_, RevokedGrantInfo>(
            r"SELECT 
                grant_id,
                revoked_at,
                revoked_by,
                reason
            FROM revoked_capability_grants
            WHERE revoked_at > $1
            ORDER BY revoked_at DESC",
        )
        .bind(since)
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(rows)
    }

    /// List all valid (non-expired) grants for a subject
    pub async fn list_grants_for_subject(
        &self,
        subject_type: &str,
        subject_id: &str,
    ) -> Result<Vec<CapabilityGrant>> {
        let rows = sqlx::query_as::<_, CapabilityGrant>(
            r"
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
            ",
        )
        .bind(subject_type)
        .bind(subject_id)
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(rows)
    }

    /// Execute a previously approved capability grant.
    ///
    /// Fetches the approval request, verifies it is approved, extracts the
    /// payload fields, and executes the original grant logic.
    pub async fn execute_approved_grant(
        &self,
        approval_id: Uuid,
        approval_queue: &ApprovalQueue,
        event_stream: Option<&EventStream>,
        ledger: Option<&Ledger>,
        owner_signing_key: Option<&SigningKey>,
    ) -> Result<Uuid> {
        let request = approval_queue.get(approval_id).await?.ok_or_else(|| {
            Error::Security(format!("Approval request not found: {}", approval_id))
        })?;

        if request.status != "approved" {
            return Err(Error::Security(format!(
                "Approval request {} is not approved (status: {})",
                approval_id, request.status
            )));
        }

        let payload = &request.payload;
        let subject_type = payload["subject_type"].as_str().ok_or_else(|| {
            Error::Security("Missing subject_type in approval payload".to_string())
        })?;
        let subject_id = payload["subject_id"]
            .as_str()
            .ok_or_else(|| Error::Security("Missing subject_id in approval payload".to_string()))?;
        let capability_key = payload["capability_key"].as_str().ok_or_else(|| {
            Error::Security("Missing capability_key in approval payload".to_string())
        })?;
        let scope = payload
            .get("scope")
            .and_then(|v| if v.is_null() { None } else { Some(v.clone()) });
        let constraints = payload
            .get("constraints")
            .and_then(|v| if v.is_null() { None } else { Some(v.clone()) });
        let approved_by = payload["approved_by"]
            .as_str()
            .and_then(|s| Uuid::parse_str(s).ok());
        let expires_at = payload["expires_at"]
            .as_str()
            .and_then(|s| s.parse::<DateTime<Utc>>().ok());

        // Execute without approval_queue to avoid recursion
        self.grant_capability(
            subject_type,
            subject_id,
            capability_key,
            scope,
            constraints,
            approved_by,
            expires_at,
            event_stream,
            ledger,
            owner_signing_key,
            None,
        )
        .await
    }

    /// Execute a previously approved capability revocation.
    ///
    /// Fetches the approval request, verifies it is approved, extracts the
    /// grant_id, and executes the original revoke logic.
    pub async fn execute_approved_revoke(
        &self,
        approval_id: Uuid,
        approval_queue: &ApprovalQueue,
        event_stream: Option<&EventStream>,
        ledger: Option<&Ledger>,
        owner_signing_key: Option<&SigningKey>,
    ) -> Result<bool> {
        let request = approval_queue.get(approval_id).await?.ok_or_else(|| {
            Error::Security(format!("Approval request not found: {}", approval_id))
        })?;

        if request.status != "approved" {
            return Err(Error::Security(format!(
                "Approval request {} is not approved (status: {})",
                approval_id, request.status
            )));
        }

        let payload = &request.payload;
        let grant_id_str = payload["grant_id"]
            .as_str()
            .ok_or_else(|| Error::Security("Missing grant_id in approval payload".to_string()))?;
        let grant_id = Uuid::parse_str(grant_id_str)
            .map_err(|e| Error::Security(format!("Invalid grant_id in approval payload: {}", e)))?;
        let revoked_by = payload["revoked_by"]
            .as_str()
            .and_then(|s| Uuid::parse_str(s).ok());

        // Execute without approval_queue to avoid recursion
        self.revoke_capability(
            grant_id,
            revoked_by,
            event_stream,
            ledger,
            owner_signing_key,
            None,
        )
        .await
    }

    /// Check if an identity can execute a task with a specific skill
    ///
    /// This verifies:
    /// 1. The identity has `task.create` capability
    /// 2. The skill has all its required capabilities granted
    ///
    /// Known limitation (v1.0.0): checks `task.create` capability and required-capability
    /// list; deeper execution-path integration deferred.
    pub async fn check_task_execution(
        &self,
        identity_id: Uuid,
        skill_id: Uuid,
        event_stream: Option<&EventStream>,
    ) -> Result<()> {
        // Check if identity has task.create capability
        if !self
            .check_capability(
                "identity",
                &identity_id.to_string(),
                "task.create",
                event_stream,
            )
            .await?
        {
            return Err(Error::Security(
                "Identity lacks task.create capability".to_string(),
            ));
        }

        // Get skill's required capabilities (TEXT[] in database)
        let required_capabilities: Option<Vec<String>> = sqlx::query_scalar!(
            r#"SELECT capabilities_required FROM skills WHERE skill_id = $1"#,
            skill_id,
        )
        .fetch_optional(&self.pool)
        .await
        .map_err(Error::Database)?
        .flatten();

        // Check each required capability for the skill
        if let Some(caps) = required_capabilities {
            for capability_key in &caps {
                if !self
                    .check_capability("skill", &skill_id.to_string(), capability_key, event_stream)
                    .await?
                {
                    return Err(Error::Security(format!(
                        "Skill lacks required capability: {}",
                        capability_key
                    )));
                }
            }
        }

        Ok(())
    }

    /// Check if an identity has a capability grant with a specific topic in scope.
    ///
    /// This checks for grants where:
    /// - capability_key = 'memory.read'
    /// - scope JSON contains the specified topic in a "topics" array
    /// - grant is not expired
    ///
    /// Used during memory import to enforce topic-scoped capability grants.
    pub async fn check_memory_topic_capability(
        &self,
        identity_id: Uuid,
        topic: &str,
    ) -> Result<bool> {
        let exists: Option<(bool,)> = sqlx::query_as(
            r"SELECT EXISTS(
                SELECT 1 FROM capability_grants
                WHERE subject_type = 'identity'
                  AND subject_id = $1
                  AND capability_key = 'memory.read'
                  AND (expires_at IS NULL OR expires_at > NOW())
                  AND scope @> jsonb_build_object('topics', jsonb_build_array($2))
            )",
        )
        .bind(identity_id.to_string())
        .bind(topic)
        .fetch_optional(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(exists.map(|(e,)| e).unwrap_or(false))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_check_capability_returns_false_for_nonexistent() {
        let db_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://carnelian:carnelian@localhost:5432/carnelian".into());
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .expect("Failed to connect to database");

        let engine = PolicyEngine::new(pool);
        let result = engine
            .check_capability(
                "identity",
                &Uuid::new_v4().to_string(),
                "nonexistent.capability",
                None,
            )
            .await
            .expect("Check should not error");

        assert!(!result, "Should return false for non-existent grant");
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_grant_and_check_capability() {
        let db_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://carnelian:carnelian@localhost:5432/carnelian".into());
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .expect("Failed to connect to database");

        let engine = PolicyEngine::new(pool);
        let subject_id = Uuid::new_v4().to_string();

        // Initially should not have capability
        let before = engine
            .check_capability("identity", &subject_id, "fs.read", None)
            .await
            .expect("Check should not error");
        assert!(!before, "Should not have capability before grant");

        // Grant the capability
        let grant_id = engine
            .grant_capability(
                "identity",
                &subject_id,
                "fs.read",
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .expect("Grant should succeed");

        // Now should have capability
        let after = engine
            .check_capability("identity", &subject_id, "fs.read", None)
            .await
            .expect("Check should not error");
        assert!(after, "Should have capability after grant");

        // Revoke and verify
        let revoked = engine
            .revoke_capability(grant_id, None, None, None, None, None)
            .await
            .expect("Revoke should not error");
        assert!(revoked, "Should have revoked the grant");

        // Should no longer have capability
        let after_revoke = engine
            .check_capability("identity", &subject_id, "fs.read", None)
            .await
            .expect("Check should not error");
        assert!(!after_revoke, "Should not have capability after revocation");
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_expired_grants_not_valid() {
        let db_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://carnelian:carnelian@localhost:5432/carnelian".into());
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .expect("Failed to connect to database");

        let engine = PolicyEngine::new(pool);
        let subject_id = Uuid::new_v4().to_string();

        // Grant with past expiration
        let past = Utc::now() - chrono::Duration::hours(1);
        let _grant_id = engine
            .grant_capability(
                "identity",
                &subject_id,
                "fs.read",
                None,
                None,
                None,
                Some(past),
                None,
                None,
                None,
                None,
            )
            .await
            .expect("Grant should succeed");

        // Should not have capability (expired)
        let result = engine
            .check_capability("identity", &subject_id, "fs.read", None)
            .await
            .expect("Check should not error");
        assert!(!result, "Expired grant should not be valid");
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_list_grants_for_subject() {
        let db_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://carnelian:carnelian@localhost:5432/carnelian".into());
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .expect("Failed to connect to database");

        let engine = PolicyEngine::new(pool);
        let subject_id = Uuid::new_v4().to_string();

        // Grant multiple capabilities
        engine
            .grant_capability(
                "identity",
                &subject_id,
                "fs.read",
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .expect("Grant should succeed");
        engine
            .grant_capability(
                "identity",
                &subject_id,
                "fs.write",
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .expect("Grant should succeed");

        // List grants
        let grants = engine
            .list_grants_for_subject("identity", &subject_id)
            .await
            .expect("List should not error");

        assert!(grants.len() >= 2, "Should have at least 2 grants");
    }
}
