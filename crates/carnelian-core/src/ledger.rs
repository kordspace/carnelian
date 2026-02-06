//! Tamper-resistant audit ledger with blake3 hash-chaining
//!
//! The `Ledger` provides an append-only audit trail where each event's hash depends
//! on the previous event's hash, creating an immutable chain. Any modification to a
//! past event will break the chain and be detected during verification.
//!
//! # Hash-Chain Architecture
//!
//! Each ledger event stores:
//! - `payload_hash`: blake3 hash of the canonical JSON payload
//! - `prev_hash`: the `event_hash` of the immediately preceding event (NULL for the first)
//! - `event_hash`: blake3 hash of `timestamp || actor_id || action_type || payload_hash || prev_hash`
//!
//! This creates a linked chain: modifying any event invalidates all subsequent hashes.
//!
//! # Verification
//!
//! On startup, `verify_chain()` replays the entire ledger and recomputes every hash,
//! ensuring no tampering has occurred. If verification fails, the server refuses to start.
//!
//! # Security Considerations
//!
//! - Phase 1 does not include digital signatures (`core_signature` is NULL).
//!   A future phase will sign each event with the owner's Ed25519 key.
//! - The hash chain protects against silent modification of stored events but does
//!   not prevent a privileged database admin from rewriting the entire chain.
//!
//! # Example
//!
//! ```ignore
//! let ledger = Ledger::new(pool);
//! ledger.load_last_hash().await?;
//!
//! let event_id = ledger.append_event(
//!     Some(actor_uuid),
//!     "capability.grant",
//!     json!({"grant_id": grant_id}),
//!     None,
//! ).await?;
//!
//! assert!(ledger.verify_chain().await?);
//! ```

use carnelian_common::{Error, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{FromRow, PgPool};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// A single event from the ledger, matching the `ledger_events` table schema.
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct LedgerEvent {
    /// Auto-incrementing event identifier
    pub event_id: i64,
    /// Timestamp of the event
    pub ts: DateTime<Utc>,
    /// Identity that performed the action (NULL for system actions)
    pub actor_id: Option<Uuid>,
    /// Type of action (e.g., "capability.grant", "config.change")
    pub action_type: String,
    /// blake3 hash of the canonical JSON payload
    pub payload_hash: String,
    /// Hash of the previous event (NULL for the first event)
    pub prev_hash: Option<String>,
    /// blake3 hash of (ts || actor_id || action_type || payload_hash || prev_hash)
    pub event_hash: String,
    /// Ed25519 signature of event_hash (NULL in Phase 1)
    pub core_signature: Option<String>,
    /// Correlation ID for tracing related operations
    pub correlation_id: Option<Uuid>,
    /// Additional metadata (JSON)
    pub metadata: Option<JsonValue>,
}

/// Tamper-resistant audit ledger backed by PostgreSQL with blake3 hash-chaining.
///
/// Each appended event's hash depends on the previous event's hash, forming an
/// immutable chain that can be verified at any time.
pub struct Ledger {
    /// Database connection pool
    pool: PgPool,
    /// Most recent event_hash, cached in memory for efficient chaining
    last_hash: Arc<RwLock<Option<String>>>,
}

impl Ledger {
    /// Create a new Ledger with the given database pool.
    ///
    /// After construction, call `load_last_hash()` to initialize the in-memory
    /// chain head from the database.
    #[must_use]
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            last_hash: Arc::new(RwLock::new(None)),
        }
    }

    /// Load the most recent `event_hash` from the database into memory.
    ///
    /// This must be called after construction and before appending events
    /// to ensure the chain continues correctly.
    pub async fn load_last_hash(&self) -> Result<Option<String>> {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT event_hash FROM ledger_events ORDER BY event_id DESC LIMIT 1")
                .fetch_optional(&self.pool)
                .await
                .map_err(Error::Database)?;

        let hash = row.map(|(h,)| h);
        (*self.last_hash.write().await).clone_from(&hash);

        tracing::debug!(last_hash = ?hash, "Loaded last ledger hash");
        Ok(hash)
    }

    /// Compute the blake3 hash of a JSON payload.
    ///
    /// The payload is serialized to a canonical JSON string before hashing,
    /// ensuring deterministic output for identical payloads.
    #[must_use]
    pub fn compute_payload_hash(payload: &JsonValue) -> String {
        let canonical = serde_json::to_string(payload).unwrap_or_default();
        let hash = blake3::hash(canonical.as_bytes());
        hex::encode(hash.as_bytes())
    }

    /// Compute the blake3 hash of a ledger event.
    ///
    /// The hash covers all critical fields concatenated as:
    /// `timestamp || actor_id || action_type || payload_hash || prev_hash`
    ///
    /// This ensures any change to any field will produce a different hash.
    #[must_use]
    pub fn compute_event_hash(
        ts: &DateTime<Utc>,
        actor_id: Option<Uuid>,
        action_type: &str,
        payload_hash: &str,
        prev_hash: Option<&str>,
    ) -> String {
        let mut input = String::new();
        input.push_str(&ts.to_rfc3339());
        input.push_str(&actor_id.map_or_else(|| "none".to_string(), |id| id.to_string()));
        input.push_str(action_type);
        input.push_str(payload_hash);
        input.push_str(prev_hash.unwrap_or("genesis"));
        let hash = blake3::hash(input.as_bytes());
        hex::encode(hash.as_bytes())
    }

    /// Append a new event to the ledger.
    ///
    /// Uses a database transaction with `SELECT ... FOR UPDATE` to obtain the
    /// authoritative `prev_hash` from the most recent row, preventing concurrent
    /// writers from forking the hash chain. The payload and event hashes are
    /// computed inside the transaction, the new row is inserted, and the
    /// in-memory chain head is updated only after a successful commit.
    ///
    /// # Arguments
    ///
    /// * `actor_id` - Identity performing the action (None for system actions)
    /// * `action_type` - Category of action (e.g., "capability.grant")
    /// * `payload` - Structured data describing the action
    /// * `correlation_id` - Optional correlation ID for tracing
    ///
    /// # Returns
    ///
    /// The generated `event_id` from the database.
    pub async fn append_event(
        &self,
        actor_id: Option<Uuid>,
        action_type: &str,
        payload: JsonValue,
        correlation_id: Option<Uuid>,
    ) -> Result<i64> {
        let payload_hash = Self::compute_payload_hash(&payload);
        let ts = Utc::now();

        // Begin a serializable transaction and lock the latest row to prevent
        // concurrent appends from reading a stale prev_hash.
        let mut tx = self.pool.begin().await.map_err(Error::Database)?;

        let prev_hash: Option<String> = sqlx::query_scalar(
            "SELECT event_hash FROM ledger_events ORDER BY event_id DESC LIMIT 1 FOR UPDATE",
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(Error::Database)?;

        let event_hash = Self::compute_event_hash(
            &ts,
            actor_id,
            action_type,
            &payload_hash,
            prev_hash.as_deref(),
        );

        let event_id: i64 = sqlx::query_scalar(
            r"
            INSERT INTO ledger_events (ts, actor_id, action_type, payload_hash, prev_hash, event_hash, correlation_id, metadata)
            VALUES ($1, $2, $3, $4, $5, $6, $7, '{}'::jsonb)
            RETURNING event_id
            ",
        )
        .bind(ts)
        .bind(actor_id)
        .bind(action_type)
        .bind(&payload_hash)
        .bind(&prev_hash)
        .bind(&event_hash)
        .bind(correlation_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(Error::Database)?;

        tx.commit().await.map_err(Error::Database)?;

        // Update in-memory cache only after successful commit
        *self.last_hash.write().await = Some(event_hash.clone());

        tracing::info!(
            event_id = event_id,
            action_type = %action_type,
            actor_id = ?actor_id,
            "Ledger event appended"
        );

        Ok(event_id)
    }

    /// Verify the integrity of the entire ledger hash chain.
    ///
    /// Replays all events in order and recomputes each hash, verifying:
    /// 1. Each event's `prev_hash` matches the preceding event's `event_hash`
    /// 2. Each event's `event_hash` matches the recomputed hash from its fields
    ///
    /// # Returns
    ///
    /// `Ok(true)` if the chain is intact, `Ok(false)` if tampering is detected.
    pub async fn verify_chain(&self) -> Result<bool> {
        let rows: Vec<LedgerEvent> = sqlx::query_as(
            "SELECT event_id, ts, actor_id, action_type, payload_hash, prev_hash, event_hash, core_signature, correlation_id, metadata FROM ledger_events ORDER BY event_id ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        if rows.is_empty() {
            tracing::info!("Ledger chain verification: empty ledger, nothing to verify");
            return Ok(true);
        }

        let mut expected_prev_hash: Option<String> = None;

        for event in &rows {
            // Verify prev_hash linkage
            if event.prev_hash != expected_prev_hash {
                tracing::error!(
                    event_id = event.event_id,
                    expected_prev_hash = ?expected_prev_hash,
                    actual_prev_hash = ?event.prev_hash,
                    "Ledger chain break: prev_hash mismatch"
                );
                return Ok(false);
            }

            // Recompute event_hash and verify
            let computed_hash = Self::compute_event_hash(
                &event.ts,
                event.actor_id,
                &event.action_type,
                &event.payload_hash,
                event.prev_hash.as_deref(),
            );

            if computed_hash != event.event_hash {
                tracing::error!(
                    event_id = event.event_id,
                    computed_hash = %computed_hash,
                    stored_hash = %event.event_hash,
                    "Ledger chain break: event_hash mismatch"
                );
                return Ok(false);
            }

            expected_prev_hash = Some(event.event_hash.clone());
        }

        tracing::info!(event_count = rows.len(), "Ledger chain verification passed");

        Ok(true)
    }

    // ── Privileged Action Logging Helpers ──────────────────────────────────

    /// Log a capability grant to the audit ledger.
    pub async fn log_capability_grant(
        &self,
        grant_id: Uuid,
        subject_type: &str,
        subject_id: &str,
        capability_key: &str,
        approved_by: Option<Uuid>,
    ) -> Result<i64> {
        self.append_event(
            approved_by,
            "capability.grant",
            serde_json::json!({
                "grant_id": grant_id,
                "subject_type": subject_type,
                "subject_id": subject_id,
                "capability_key": capability_key,
            }),
            None,
        )
        .await
    }

    /// Log a capability revocation to the audit ledger.
    pub async fn log_capability_revoke(
        &self,
        grant_id: Uuid,
        revoked_by: Option<Uuid>,
    ) -> Result<i64> {
        self.append_event(
            revoked_by,
            "capability.revoke",
            serde_json::json!({
                "grant_id": grant_id,
            }),
            None,
        )
        .await
    }

    /// Log a configuration change to the audit ledger.
    pub async fn log_config_change(
        &self,
        config_key: &str,
        old_value: Option<&JsonValue>,
        new_value: &JsonValue,
        changed_by: Option<Uuid>,
    ) -> Result<i64> {
        self.append_event(
            changed_by,
            "config.change",
            serde_json::json!({
                "config_key": config_key,
                "old_value": old_value,
                "new_value": new_value,
            }),
            None,
        )
        .await
    }

    /// Log a database migration to the audit ledger.
    pub async fn log_migration(&self, migration_version: &str) -> Result<i64> {
        self.append_event(
            None,
            "db.migration",
            serde_json::json!({
                "migration_version": migration_version,
            }),
            None,
        )
        .await
    }

    // ── Query Methods ─────────────────────────────────────────────────────

    /// Query ledger events by actor ID.
    pub async fn get_events_by_actor(
        &self,
        actor_id: Uuid,
        limit: i64,
    ) -> Result<Vec<LedgerEvent>> {
        let rows: Vec<LedgerEvent> = sqlx::query_as(
            "SELECT event_id, ts, actor_id, action_type, payload_hash, prev_hash, event_hash, core_signature, correlation_id, metadata FROM ledger_events WHERE actor_id = $1 ORDER BY ts DESC LIMIT $2",
        )
        .bind(actor_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(rows)
    }

    /// Query ledger events by action type.
    pub async fn get_events_by_action_type(
        &self,
        action_type: &str,
        limit: i64,
    ) -> Result<Vec<LedgerEvent>> {
        let rows: Vec<LedgerEvent> = sqlx::query_as(
            "SELECT event_id, ts, actor_id, action_type, payload_hash, prev_hash, event_hash, core_signature, correlation_id, metadata FROM ledger_events WHERE action_type = $1 ORDER BY ts DESC LIMIT $2",
        )
        .bind(action_type)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(rows)
    }

    /// Query ledger events by correlation ID.
    pub async fn get_events_by_correlation(
        &self,
        correlation_id: Uuid,
    ) -> Result<Vec<LedgerEvent>> {
        let rows: Vec<LedgerEvent> = sqlx::query_as(
            "SELECT event_id, ts, actor_id, action_type, payload_hash, prev_hash, event_hash, core_signature, correlation_id, metadata FROM ledger_events WHERE correlation_id = $1 ORDER BY ts ASC",
        )
        .bind(correlation_id)
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(rows)
    }

    /// Query the most recent ledger events.
    pub async fn get_recent_events(&self, limit: i64) -> Result<Vec<LedgerEvent>> {
        let rows: Vec<LedgerEvent> = sqlx::query_as(
            "SELECT event_id, ts, actor_id, action_type, payload_hash, prev_hash, event_hash, core_signature, correlation_id, metadata FROM ledger_events ORDER BY ts DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(rows)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_payload_hash_deterministic() {
        let payload = serde_json::json!({"key": "value", "number": 42});
        let hash1 = Ledger::compute_payload_hash(&payload);
        let hash2 = Ledger::compute_payload_hash(&payload);
        assert_eq!(hash1, hash2, "Same payload must produce same hash");
        assert_eq!(hash1.len(), 64, "blake3 hex hash should be 64 characters");
    }

    #[test]
    fn test_compute_payload_hash_different_payloads() {
        let payload1 = serde_json::json!({"key": "value1"});
        let payload2 = serde_json::json!({"key": "value2"});
        let hash1 = Ledger::compute_payload_hash(&payload1);
        let hash2 = Ledger::compute_payload_hash(&payload2);
        assert_ne!(
            hash1, hash2,
            "Different payloads must produce different hashes"
        );
    }

    #[test]
    fn test_compute_event_hash_includes_all_fields() {
        let ts = Utc::now();
        let actor = Some(Uuid::new_v4());
        let action = "test.action";
        let payload_hash = "abc123";
        let prev_hash = Some("def456");

        let base_hash = Ledger::compute_event_hash(&ts, actor, action, payload_hash, prev_hash);

        // Changing any field should produce a different hash
        let different_action =
            Ledger::compute_event_hash(&ts, actor, "other.action", payload_hash, prev_hash);
        assert_ne!(base_hash, different_action);

        let different_actor =
            Ledger::compute_event_hash(&ts, Some(Uuid::new_v4()), action, payload_hash, prev_hash);
        assert_ne!(base_hash, different_actor);

        let different_prev =
            Ledger::compute_event_hash(&ts, actor, action, payload_hash, Some("other"));
        assert_ne!(base_hash, different_prev);

        let no_prev = Ledger::compute_event_hash(&ts, actor, action, payload_hash, None);
        assert_ne!(base_hash, no_prev);
    }

    #[test]
    fn test_compute_event_hash_genesis() {
        let ts = Utc::now();
        let hash = Ledger::compute_event_hash(&ts, None, "genesis", "empty", None);
        assert_eq!(hash.len(), 64);
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_append_event_updates_last_hash() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect("postgresql://carnelian:carnelian@localhost:5432/carnelian")
            .await
            .expect("Failed to connect to database");

        let ledger = Ledger::new(pool);
        ledger
            .load_last_hash()
            .await
            .expect("load_last_hash failed");

        let event_id = ledger
            .append_event(None, "test.append", serde_json::json!({"test": true}), None)
            .await
            .expect("append_event failed");

        assert!(event_id > 0);
        let last = ledger.last_hash.read().await.clone();
        assert!(last.is_some(), "last_hash should be set after append");
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_verify_chain_empty_ledger() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect("postgresql://carnelian:carnelian@localhost:5432/carnelian")
            .await
            .expect("Failed to connect to database");

        // Use a fresh database or accept existing chain
        let ledger = Ledger::new(pool);
        let result = ledger.verify_chain().await.expect("verify_chain failed");
        assert!(result, "Chain should verify (empty or valid)");
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_verify_chain_multiple_events() {
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect("postgresql://carnelian:carnelian@localhost:5432/carnelian")
            .await
            .expect("Failed to connect to database");

        let ledger = Ledger::new(pool);
        ledger
            .load_last_hash()
            .await
            .expect("load_last_hash failed");

        // Append several events
        for i in 0..3 {
            ledger
                .append_event(
                    None,
                    "test.chain",
                    serde_json::json!({"iteration": i}),
                    None,
                )
                .await
                .expect("append_event failed");
        }

        let result = ledger.verify_chain().await.expect("verify_chain failed");
        assert!(result, "Chain should verify after multiple appends");
    }
}
