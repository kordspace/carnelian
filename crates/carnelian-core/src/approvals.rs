//! Approval queue for privileged actions requiring owner authorization
//!
//! This module provides a database-backed approval queue that intercepts privileged
//! actions (capability grants, config changes, migrations) before execution. Approval
//! and denial operations require Ed25519 signatures from the owner's signing key,
//! verified against the stored public key.
//!
//! # Workflow
//!
//! 1. A privileged action is requested (e.g., capability grant)
//! 2. The action is queued in `approval_queue` with status `pending`
//! 3. The owner reviews and approves/denies with their signing key
//! 4. The decision is cryptographically signed and recorded in the ledger
//! 5. If approved, the original action is executed
//!
//! # Security
//!
//! - All approval/denial decisions are signed with Ed25519
//! - Signatures are stored as hex-encoded strings in the database
//! - All decisions are logged to the tamper-resistant audit ledger
//! - Database transactions with `FOR UPDATE` locks prevent concurrent modifications

use carnelian_common::{Error, Result};
use chrono::{DateTime, Utc};
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::crypto;
use crate::ledger::Ledger;

/// Represents a queued approval request from the database
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ApprovalRequest {
    /// Unique identifier for this approval request
    pub id: Uuid,
    /// Type of privileged action (e.g., "capability.grant", "config.change", "db.migration")
    pub action_type: String,
    /// Action-specific data stored as JSONB
    pub payload: JsonValue,
    /// Current status: "pending", "approved", or "denied"
    pub status: String,
    /// Identity that requested the action
    pub requested_by: Option<Uuid>,
    /// When the request was created
    pub requested_at: DateTime<Utc>,
    /// When the request was resolved (approved/denied)
    pub resolved_at: Option<DateTime<Utc>>,
    /// Identity that resolved the request
    pub resolved_by: Option<Uuid>,
    /// Hex-encoded Ed25519 signature of the approval/denial decision
    pub signature: Option<String>,
    /// Correlation ID for end-to-end tracing
    pub correlation_id: Option<Uuid>,
}

/// Database-backed approval queue for privileged actions
pub struct ApprovalQueue {
    pool: PgPool,
}

impl ApprovalQueue {
    /// Create a new ApprovalQueue with a database connection pool
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Queue a privileged action for approval.
    ///
    /// Inserts a new approval request with status `pending` and returns the
    /// generated approval ID for tracking.
    pub async fn queue_action(
        &self,
        action_type: &str,
        payload: JsonValue,
        requested_by: Option<Uuid>,
        correlation_id: Option<Uuid>,
    ) -> Result<Uuid> {
        let approval_id: Uuid = sqlx::query_scalar(
            r"INSERT INTO approval_queue (action_type, payload, requested_by, correlation_id)
              VALUES ($1, $2, $3, $4)
              RETURNING id",
        )
        .bind(action_type)
        .bind(&payload)
        .bind(requested_by)
        .bind(correlation_id)
        .fetch_one(&self.pool)
        .await
        .map_err(Error::Database)?;

        tracing::info!(
            approval_id = %approval_id,
            action_type = %action_type,
            correlation_id = ?correlation_id,
            "Privileged action queued for approval"
        );

        Ok(approval_id)
    }

    /// List all pending approval requests, ordered by most recent first.
    pub async fn list_pending(&self, limit: i64) -> Result<Vec<ApprovalRequest>> {
        let rows = sqlx::query_as::<_, ApprovalRequest>(
            r"SELECT id, action_type, payload, status, requested_by, requested_at,
                     resolved_at, resolved_by, signature, correlation_id
              FROM approval_queue
              WHERE status = 'pending'
              ORDER BY requested_at DESC
              LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(rows)
    }

    /// Approve a pending action with an Ed25519 signature.
    ///
    /// This method:
    /// 1. Acquires a `FOR UPDATE` lock on the approval row
    /// 2. Verifies the request is still pending
    /// 3. Signs the approval_id with the owner's signing key
    /// 4. Updates the row with status, signature, and resolver info
    /// 5. Logs the decision to the audit ledger
    pub async fn approve(
        &self,
        approval_id: Uuid,
        approved_by: Uuid,
        owner_signing_key: &SigningKey,
        ledger: &Ledger,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(Error::Database)?;

        // Fetch with FOR UPDATE lock
        let row: Option<ApprovalRequest> = sqlx::query_as(
            r"SELECT id, action_type, payload, status, requested_by, requested_at,
                     resolved_at, resolved_by, signature, correlation_id
              FROM approval_queue
              WHERE id = $1
              FOR UPDATE",
        )
        .bind(approval_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(Error::Database)?;

        let request = row.ok_or_else(|| {
            Error::Security(format!("Approval request not found: {}", approval_id))
        })?;

        if request.status != "pending" {
            return Err(Error::Security(format!(
                "Approval request {} is already {}, cannot approve",
                approval_id, request.status
            )));
        }

        // Sign the approval_id as proof of authorization
        let signature = crypto::sign_bytes(owner_signing_key, approval_id.to_string().as_bytes());

        // Update the row
        sqlx::query(
            r"UPDATE approval_queue
              SET status = 'approved', resolved_at = NOW(), resolved_by = $1, signature = $2
              WHERE id = $3",
        )
        .bind(approved_by)
        .bind(&signature)
        .bind(approval_id)
        .execute(&mut *tx)
        .await
        .map_err(Error::Database)?;

        tx.commit().await.map_err(Error::Database)?;

        tracing::info!(
            approval_id = %approval_id,
            approved_by = %approved_by,
            action_type = %request.action_type,
            "Approval granted"
        );

        // Log to audit ledger with cryptographic signature
        if let Err(e) = ledger
            .append_event(
                Some(approved_by),
                "approval.granted",
                json!({
                    "approval_id": approval_id,
                    "action_type": request.action_type,
                    "correlation_id": request.correlation_id,
                }),
                request.correlation_id,
                Some(owner_signing_key),
                None,
                None,
                None,
            )
            .await
        {
            tracing::warn!(error = %e, "Failed to log approval.granted to ledger");
        }

        Ok(())
    }

    /// Deny a pending action with an Ed25519 signature.
    ///
    /// Similar to `approve()` but sets status to `denied` and logs
    /// `approval.denied` to the audit ledger.
    pub async fn deny(
        &self,
        approval_id: Uuid,
        denied_by: Uuid,
        owner_signing_key: &SigningKey,
        ledger: &Ledger,
    ) -> Result<()> {
        let mut tx = self.pool.begin().await.map_err(Error::Database)?;

        // Fetch with FOR UPDATE lock
        let row: Option<ApprovalRequest> = sqlx::query_as(
            r"SELECT id, action_type, payload, status, requested_by, requested_at,
                     resolved_at, resolved_by, signature, correlation_id
              FROM approval_queue
              WHERE id = $1
              FOR UPDATE",
        )
        .bind(approval_id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(Error::Database)?;

        let request = row.ok_or_else(|| {
            Error::Security(format!("Approval request not found: {}", approval_id))
        })?;

        if request.status != "pending" {
            return Err(Error::Security(format!(
                "Approval request {} is already {}, cannot deny",
                approval_id, request.status
            )));
        }

        // Sign the denial
        let signature = crypto::sign_bytes(owner_signing_key, approval_id.to_string().as_bytes());

        // Update the row
        sqlx::query(
            r"UPDATE approval_queue
              SET status = 'denied', resolved_at = NOW(), resolved_by = $1, signature = $2
              WHERE id = $3",
        )
        .bind(denied_by)
        .bind(&signature)
        .bind(approval_id)
        .execute(&mut *tx)
        .await
        .map_err(Error::Database)?;

        tx.commit().await.map_err(Error::Database)?;

        tracing::info!(
            approval_id = %approval_id,
            denied_by = %denied_by,
            action_type = %request.action_type,
            "Approval denied"
        );

        // Log to audit ledger with cryptographic signature
        if let Err(e) = ledger
            .append_event(
                Some(denied_by),
                "approval.denied",
                json!({
                    "approval_id": approval_id,
                    "action_type": request.action_type,
                    "correlation_id": request.correlation_id,
                }),
                request.correlation_id,
                Some(owner_signing_key),
                None,
                None,
                None,
            )
            .await
        {
            tracing::warn!(error = %e, "Failed to log approval.denied to ledger");
        }

        Ok(())
    }

    /// Approve multiple pending actions in batch.
    ///
    /// Processes each approval individually, collecting successes. Partial
    /// failures are logged but do not abort the batch.
    ///
    /// Returns the list of successfully approved IDs.
    pub async fn batch_approve(
        &self,
        approval_ids: Vec<Uuid>,
        approved_by: Uuid,
        owner_signing_key: &SigningKey,
        ledger: &Ledger,
    ) -> Result<Vec<Uuid>> {
        let mut approved = Vec::with_capacity(approval_ids.len());

        for id in &approval_ids {
            match self
                .approve(*id, approved_by, owner_signing_key, ledger)
                .await
            {
                Ok(()) => approved.push(*id),
                Err(e) => {
                    tracing::warn!(
                        approval_id = %id,
                        error = %e,
                        "Failed to approve in batch, continuing"
                    );
                }
            }
        }

        tracing::info!(
            total = approval_ids.len(),
            approved = approved.len(),
            "Batch approval completed"
        );

        Ok(approved)
    }

    /// Verify the Ed25519 signature on a resolved approval request.
    ///
    /// Returns `true` if the stored signature is valid for the approval_id
    /// when verified against the provided public key.
    pub async fn verify_approval_signature(
        &self,
        approval_id: Uuid,
        public_key_hex: &str,
    ) -> Result<bool> {
        let row: Option<ApprovalRequest> = sqlx::query_as(
            r"SELECT id, action_type, payload, status, requested_by, requested_at,
                     resolved_at, resolved_by, signature, correlation_id
              FROM approval_queue
              WHERE id = $1",
        )
        .bind(approval_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(Error::Database)?;

        let request = row.ok_or_else(|| {
            Error::Security(format!("Approval request not found: {}", approval_id))
        })?;

        let signature_hex = request.signature.ok_or_else(|| {
            Error::Security(format!(
                "Approval request {} has no signature (status: {})",
                approval_id, request.status
            ))
        })?;

        crypto::verify_signature(
            public_key_hex,
            approval_id.to_string().as_bytes(),
            &signature_hex,
        )
    }

    /// Fetch a single approval request by ID.
    pub async fn get(&self, approval_id: Uuid) -> Result<Option<ApprovalRequest>> {
        let row = sqlx::query_as::<_, ApprovalRequest>(
            r"SELECT id, action_type, payload, status, requested_by, requested_at,
                     resolved_at, resolved_by, signature, correlation_id
              FROM approval_queue
              WHERE id = $1",
        )
        .bind(approval_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(row)
    }
}

/// Check if an action type requires approval before execution.
///
/// Matches the privileged action list from `ledger::is_privileged_action`.
pub fn is_privileged_action(action_type: &str) -> bool {
    matches!(
        action_type,
        "capability.grant" | "capability.revoke" | "config.change" | "db.migration"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_privileged_action() {
        assert!(is_privileged_action("capability.grant"));
        assert!(is_privileged_action("capability.revoke"));
        assert!(is_privileged_action("config.change"));
        assert!(is_privileged_action("db.migration"));
        assert!(!is_privileged_action("session.created"));
        assert!(!is_privileged_action("heartbeat.completed"));
        assert!(!is_privileged_action("model.call.request"));
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_queue_and_list_pending() {
        let db_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://carnelian:carnelian@localhost:5432/carnelian".into());
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect(&db_url)
            .await
            .expect("Failed to connect to database");

        let queue = ApprovalQueue::new(pool);
        let corr = Uuid::now_v7();

        let id1 = queue
            .queue_action(
                "capability.grant",
                json!({"subject_type": "identity", "capability_key": "fs.read"}),
                None,
                Some(corr),
            )
            .await
            .expect("queue_action should succeed");

        let id2 = queue
            .queue_action(
                "config.change",
                json!({"key": "model.default", "new_value": "gpt-4"}),
                None,
                None,
            )
            .await
            .expect("queue_action should succeed");

        let pending = queue
            .list_pending(10)
            .await
            .expect("list_pending should succeed");
        let ids: Vec<Uuid> = pending.iter().map(|r| r.id).collect();
        assert!(ids.contains(&id1), "Should contain first queued item");
        assert!(ids.contains(&id2), "Should contain second queued item");

        // Verify ordering (most recent first)
        if let Some(pos1) = ids.iter().position(|&x| x == id1) {
            if let Some(pos2) = ids.iter().position(|&x| x == id2) {
                assert!(pos2 < pos1, "More recent item should appear first");
            }
        }
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_approve_with_signature() {
        let db_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://carnelian:carnelian@localhost:5432/carnelian".into());
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect(&db_url)
            .await
            .expect("Failed to connect to database");

        let queue = ApprovalQueue::new(pool.clone());
        let ledger = Ledger::new(pool.clone());
        let (signing_key, _) = crypto::generate_ed25519_keypair();
        let approver_id = Uuid::now_v7();

        let approval_id = queue
            .queue_action("capability.grant", json!({"test": true}), None, None)
            .await
            .expect("queue should succeed");

        queue
            .approve(approval_id, approver_id, &signing_key, &ledger)
            .await
            .expect("approve should succeed");

        let request = queue
            .get(approval_id)
            .await
            .expect("get should succeed")
            .expect("request should exist");
        assert_eq!(request.status, "approved");
        assert!(request.signature.is_some());
        assert_eq!(request.resolved_by, Some(approver_id));
        assert!(request.resolved_at.is_some());
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_deny_with_signature() {
        let db_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://carnelian:carnelian@localhost:5432/carnelian".into());
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect(&db_url)
            .await
            .expect("Failed to connect to database");

        let queue = ApprovalQueue::new(pool.clone());
        let ledger = Ledger::new(pool.clone());
        let (signing_key, _) = crypto::generate_ed25519_keypair();
        let denier_id = Uuid::now_v7();

        let approval_id = queue
            .queue_action("config.change", json!({"key": "test"}), None, None)
            .await
            .expect("queue should succeed");

        queue
            .deny(approval_id, denier_id, &signing_key, &ledger)
            .await
            .expect("deny should succeed");

        let request = queue
            .get(approval_id)
            .await
            .expect("get should succeed")
            .expect("request should exist");
        assert_eq!(request.status, "denied");
        assert!(request.signature.is_some());
        assert_eq!(request.resolved_by, Some(denier_id));
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_batch_approve() {
        let db_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://carnelian:carnelian@localhost:5432/carnelian".into());
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect(&db_url)
            .await
            .expect("Failed to connect to database");

        let queue = ApprovalQueue::new(pool.clone());
        let ledger = Ledger::new(pool.clone());
        let (signing_key, _) = crypto::generate_ed25519_keypair();
        let approver_id = Uuid::now_v7();

        let id1 = queue
            .queue_action("capability.grant", json!({"cap": "fs.read"}), None, None)
            .await
            .expect("queue should succeed");
        let id2 = queue
            .queue_action("capability.grant", json!({"cap": "fs.write"}), None, None)
            .await
            .expect("queue should succeed");
        let id3 = queue
            .queue_action("config.change", json!({"key": "test"}), None, None)
            .await
            .expect("queue should succeed");

        let approved = queue
            .batch_approve(vec![id1, id2, id3], approver_id, &signing_key, &ledger)
            .await
            .expect("batch_approve should succeed");

        assert_eq!(approved.len(), 3, "All three should be approved");
        assert!(approved.contains(&id1));
        assert!(approved.contains(&id2));
        assert!(approved.contains(&id3));
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_verify_approval_signature() {
        let db_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://carnelian:carnelian@localhost:5432/carnelian".into());
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect(&db_url)
            .await
            .expect("Failed to connect to database");

        let queue = ApprovalQueue::new(pool.clone());
        let ledger = Ledger::new(pool.clone());
        let (signing_key, _) = crypto::generate_ed25519_keypair();
        let public_hex = crypto::public_key_from_signing_key(&signing_key);
        let approver_id = Uuid::now_v7();

        let approval_id = queue
            .queue_action("capability.grant", json!({"test": true}), None, None)
            .await
            .expect("queue should succeed");

        queue
            .approve(approval_id, approver_id, &signing_key, &ledger)
            .await
            .expect("approve should succeed");

        // Verify with correct key
        let valid = queue
            .verify_approval_signature(approval_id, &public_hex)
            .await
            .expect("verify should succeed");
        assert!(valid, "Signature should verify with correct key");

        // Verify with wrong key
        let (wrong_key, _) = crypto::generate_ed25519_keypair();
        let wrong_hex = crypto::public_key_from_signing_key(&wrong_key);
        let invalid = queue
            .verify_approval_signature(approval_id, &wrong_hex)
            .await
            .expect("verify should succeed");
        assert!(!invalid, "Signature should fail with wrong key");
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_cannot_approve_already_resolved() {
        let db_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://carnelian:carnelian@localhost:5432/carnelian".into());
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(2)
            .connect(&db_url)
            .await
            .expect("Failed to connect to database");

        let queue = ApprovalQueue::new(pool.clone());
        let ledger = Ledger::new(pool.clone());
        let (signing_key, _) = crypto::generate_ed25519_keypair();
        let actor = Uuid::now_v7();

        let approval_id = queue
            .queue_action("capability.grant", json!({"test": true}), None, None)
            .await
            .expect("queue should succeed");

        // Approve once
        queue
            .approve(approval_id, actor, &signing_key, &ledger)
            .await
            .expect("first approve should succeed");

        // Second approve should fail
        let result = queue
            .approve(approval_id, actor, &signing_key, &ledger)
            .await;
        assert!(
            result.is_err(),
            "Should not approve already-approved request"
        );
    }
}
