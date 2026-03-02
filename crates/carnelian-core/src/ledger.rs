//! Tamper-resistant audit ledger with blake3 hash-chaining and Ed25519 signatures
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
//! - `core_signature`: Ed25519 signature of `event_hash` for privileged actions
//!
//! This creates a linked chain: modifying any event invalidates all subsequent hashes.
//!
//! # Privileged Action Signatures
//!
//! Actions such as `capability.grant`, `capability.revoke`, `config.change`, and
//! `db.migration` are considered privileged. When an owner signing key is available,
//! these events are signed with Ed25519, and the hex-encoded signature is stored in
//! `core_signature`. On verification, the signature is checked against the owner's
//! public key, providing cryptographic proof of authorization.
//!
//! # Verification
//!
//! On startup, `verify_chain()` replays the entire ledger and recomputes every hash,
//! ensuring no tampering has occurred. If an owner public key is provided, Ed25519
//! signatures on privileged actions are also verified. If verification fails, the
//! server refuses to start.
//!
//! # Security Considerations
//!
//! - The hash chain protects against silent modification of stored events but does
//!   not prevent a privileged database admin from rewriting the entire chain.
//! - Ed25519 signatures on privileged actions add cryptographic non-repudiation:
//!   even a database admin cannot forge a valid signature without the owner's key.
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
//!     config.owner_signing_key.as_ref(),
//!     None, // metadata
//! ).await?;
//!
//! assert!(ledger.verify_chain(config.owner_public_key.as_deref()).await?);
//! ```

use carnelian_common::{Error, Result};
use carnelian_magic::{EntropyProvider, MixedEntropyProvider};
// Import Arc impl to enable EntropyProvider methods on Arc<MixedEntropyProvider>
use carnelian_magic::entropy_arc_impl as _;
use chrono::{DateTime, Timelike, Utc};
use ed25519_dalek::SigningKey;
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{FromRow, PgPool, Row};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::memory::ChainAnchor;

/// Check whether an action type is considered privileged and requires Ed25519 signing.
///
/// Privileged actions are those that modify security-critical state (capabilities,
/// configuration, database schema) or approval workflows. When an owner signing key
/// is available, these actions are signed to provide cryptographic non-repudiation.
fn is_privileged_action(action_type: &str) -> bool {
    matches!(
        action_type,
        "capability.grant"
            | "capability.revoke"
            | "config.change"
            | "db.migration"
            | "approval.granted"
            | "approval.denied"
            | "safe_mode.enabled"
            | "safe_mode.disabled"
            | "worker.quarantined"
    )
}

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
    /// 16-byte quantum salt from MixedEntropyProvider (NULL when MAGIC is disabled)
    pub quantum_salt: Option<Vec<u8>>,
}

/// Tamper-resistant audit ledger backed by PostgreSQL with blake3 hash-chaining.
///
/// Each appended event's hash depends on the previous event's hash, forming an
/// immutable chain that can be verified at any time.
#[derive(Clone)]
pub struct Ledger {
    /// Database connection pool
    pool: PgPool,
    /// Most recent event_hash, cached in memory for efficient chaining
    last_hash: Arc<RwLock<Option<String>>>,
    /// Optional XP manager for awarding XP on privileged actions
    xp_manager: Option<Arc<crate::xp::XpManager>>,
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
            xp_manager: None,
        }
    }

    /// Set the XP manager for awarding XP on privileged ledger actions.
    pub fn set_xp_manager(&mut self, xp_manager: Arc<crate::xp::XpManager>) {
        self.xp_manager = Some(xp_manager);
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
    /// `timestamp || actor_id || action_type || payload_hash || prev_hash || quantum_salt`
    ///
    /// This ensures any change to any field will produce a different hash.
    #[must_use]
    pub fn compute_event_hash(
        ts: &DateTime<Utc>,
        actor_id: Option<Uuid>,
        action_type: &str,
        payload_hash: &str,
        prev_hash: Option<&str>,
        quantum_salt: Option<&[u8]>,
    ) -> String {
        let mut input = String::new();
        input.push_str(&ts.to_rfc3339());
        input.push_str(&actor_id.map_or_else(|| "none".to_string(), |id| id.to_string()));
        input.push_str(action_type);
        input.push_str(payload_hash);
        input.push_str(prev_hash.unwrap_or("genesis"));
        input.push_str(&quantum_salt.map_or_else(|| "no-salt".to_string(), hex::encode));
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
    /// If the action is privileged and an owner signing key is provided, the
    /// event hash is signed with Ed25519 and stored in `core_signature`.
    ///
    /// # Arguments
    ///
    /// * `actor_id` - Identity performing the action (None for system actions)
    /// * `action_type` - Category of action (e.g., "capability.grant")
    /// * `payload` - Structured data describing the action
    /// * `correlation_id` - Optional correlation ID for tracing
    /// * `owner_signing_key` - Optional Ed25519 key for signing privileged actions
    /// * `metadata` - Optional JSONB metadata persisted alongside the event (e.g., provenance details)
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
        owner_signing_key: Option<&SigningKey>,
        metadata: Option<JsonValue>,
        quantum_salt: Option<Vec<u8>>,
        entropy_provider: Option<&Arc<carnelian_magic::MixedEntropyProvider>>,
    ) -> Result<i64> {
        let payload_hash = Self::compute_payload_hash(&payload);
        // Truncate to microsecond precision to match PostgreSQL TIMESTAMPTZ storage.
        // Without this, nanoseconds are lost on DB round-trip, breaking hash verification.
        let ts = {
            let now = Utc::now();
            now.with_nanosecond((now.nanosecond() / 1_000) * 1_000)
                .unwrap_or(now)
        };

        // Begin a serializable transaction and lock the latest row to prevent
        // concurrent appends from reading a stale prev_hash.
        let mut tx = self.pool.begin().await.map_err(Error::Database)?;

        let prev_hash: Option<String> = sqlx::query_scalar(
            "SELECT event_hash FROM ledger_events ORDER BY event_id DESC LIMIT 1 FOR UPDATE",
        )
        .fetch_optional(&mut *tx)
        .await
        .map_err(Error::Database)?;

        // Resolve quantum salt: use provided salt, or generate from entropy provider
        let (resolved_salt, entropy_latency_ms) = if let Some(salt) = quantum_salt {
            (Some(salt), None)
        } else if let Some(provider) = entropy_provider {
            let start = std::time::Instant::now();
            match provider.as_ref().get_bytes(16).await {
                Ok(salt_bytes) => {
                    let latency_ms = start.elapsed().as_millis() as i64;
                    tracing::debug!(
                        latency_ms = latency_ms,
                        "Generated quantum salt for ledger event"
                    );
                    (Some(salt_bytes), Some(latency_ms))
                }
                Err(e) => {
                    let latency_ms = start.elapsed().as_millis() as i64;
                    tracing::warn!(error = %e, latency_ms = latency_ms, "Failed to generate quantum salt, proceeding without");
                    (None, Some(latency_ms))
                }
            }
        } else {
            (None, None)
        };

        let event_hash = Self::compute_event_hash(
            &ts,
            actor_id,
            action_type,
            &payload_hash,
            prev_hash.as_deref(),
            resolved_salt.as_deref(),
        );

        // Sign privileged actions with the owner's Ed25519 key
        let core_signature: Option<String> = if is_privileged_action(action_type) {
            owner_signing_key.map(|signing_key| {
                let sig = crate::crypto::sign_bytes(signing_key, event_hash.as_bytes());
                tracing::info!(
                    action_type = %action_type,
                    "Privileged ledger event signed with owner key"
                );
                sig
            })
        } else {
            None
        };

        let metadata_value = metadata.unwrap_or_else(|| serde_json::json!({}));

        let event_id: i64 = sqlx::query_scalar(
            r"
            INSERT INTO ledger_events (ts, actor_id, action_type, payload_hash, prev_hash, event_hash, core_signature, correlation_id, metadata, quantum_salt)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            RETURNING event_id
            ",
        )
        .bind(ts)
        .bind(actor_id)
        .bind(action_type)
        .bind(&payload_hash)
        .bind(&prev_hash)
        .bind(&event_hash)
        .bind(&core_signature)
        .bind(correlation_id)
        .bind(&metadata_value)
        .bind(&resolved_salt)
        .fetch_one(&mut *tx)
        .await
        .map_err(Error::Database)?;

        tx.commit().await.map_err(Error::Database)?;

        // Update in-memory cache only after successful commit
        *self.last_hash.write().await = Some(event_hash.clone());

        // Log entropy event to magic_entropy_log for every request (success and failure)
        if entropy_provider.is_some() {
            let log_id = Uuid::now_v7();
            let source = "mixed";
            let bytes_requested = 16i32;
            let quantum_available = resolved_salt.is_some();
            let latency_ms = entropy_latency_ms;
            
            let pool = self.pool.clone();
            let source_owned = source.to_string();
            let correlation_id_owned = correlation_id;
            tokio::spawn(async move {
                let _ = sqlx::query(
                    r"INSERT INTO magic_entropy_log (log_id, ts, source, bytes_requested, quantum_available, latency_ms, correlation_id)
                      VALUES ($1, NOW(), $2, $3, $4, $5, $6)"
                )
                .bind(log_id)
                .bind(source_owned)
                .bind(bytes_requested)
                .bind(quantum_available)
                .bind(latency_ms)
                .bind(correlation_id_owned)
                .execute(&pool)
                .await;
            });
        }

        tracing::info!(
            event_id = event_id,
            action_type = %action_type,
            actor_id = ?actor_id,
            signed = core_signature.is_some(),
            quantum_salt = resolved_salt.is_some(),
            "Ledger event appended"
        );

        // Award XP for privileged actions (fire-and-forget)
        if is_privileged_action(action_type) {
            if let (Some(actor), Some(xp_mgr)) = (actor_id, &self.xp_manager) {
                let xp_amount = crate::xp::XpManager::calculate_ledger_xp(action_type);
                let source = crate::xp::XpSource::LedgerSigning {
                    ledger_event_id: event_id,
                };
                if let Err(e) = xp_mgr.award_xp(actor, source, xp_amount, None).await {
                    tracing::warn!(error = %e, "Failed to award ledger XP");
                }
            }
        }

        Ok(event_id)
    }

    /// Verify the integrity of the entire ledger hash chain.
    ///
    /// Replays all events in order and recomputes each hash, verifying:
    /// 1. Each event's `prev_hash` matches the preceding event's `event_hash`
    /// 2. Each event's `event_hash` matches the recomputed hash from its fields
    /// 3. If `owner_public_key` is provided, Ed25519 signatures on privileged
    ///    actions are verified against the owner's public key
    ///
    /// # Arguments
    ///
    /// * `owner_public_key` - Optional hex-encoded Ed25519 public key for signature verification
    ///
    /// # Returns
    ///
    /// `Ok(true)` if the chain is intact, `Ok(false)` if tampering is detected.
    pub async fn verify_chain(&self, owner_public_key: Option<&str>) -> Result<bool> {
        let rows: Vec<LedgerEvent> = sqlx::query_as(
            "SELECT event_id, ts, actor_id, action_type, payload_hash, prev_hash, event_hash, core_signature, correlation_id, metadata, quantum_salt FROM ledger_events ORDER BY event_id ASC",
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
                event.quantum_salt.as_deref(),
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

            // Verify Ed25519 signature if present
            if let Some(ref signature) = event.core_signature {
                if let Some(pub_key) = owner_public_key {
                    match crate::crypto::verify_signature(
                        pub_key,
                        event.event_hash.as_bytes(),
                        signature,
                    ) {
                        Ok(true) => { /* signature valid */ }
                        Ok(false) => {
                            tracing::error!(
                                event_id = event.event_id,
                                action_type = %event.action_type,
                                "Ledger chain break: invalid Ed25519 signature on event"
                            );
                            return Ok(false);
                        }
                        Err(e) => {
                            tracing::error!(
                                event_id = event.event_id,
                                error = %e,
                                "Ledger chain break: signature verification error"
                            );
                            return Ok(false);
                        }
                    }
                } else {
                    tracing::warn!(
                        event_id = event.event_id,
                        action_type = %event.action_type,
                        "Signed event cannot be verified: owner public key not loaded"
                    );
                }
            } else if is_privileged_action(&event.action_type) {
                tracing::warn!(
                    event_id = event.event_id,
                    action_type = %event.action_type,
                    "Unsigned privileged action detected (backward compatibility)"
                );
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
        owner_signing_key: Option<&SigningKey>,
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
            owner_signing_key,
            None,
            None,
            None,
        )
        .await
    }

    /// Log a capability revocation to the audit ledger.
    pub async fn log_capability_revoke(
        &self,
        grant_id: Uuid,
        revoked_by: Option<Uuid>,
        owner_signing_key: Option<&SigningKey>,
    ) -> Result<i64> {
        self.append_event(
            revoked_by,
            "capability.revoke",
            serde_json::json!({
                "grant_id": grant_id,
            }),
            None,
            owner_signing_key,
            None,
            None,
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
        owner_signing_key: Option<&SigningKey>,
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
            owner_signing_key,
            None,
            None,
            None,
        )
        .await
    }

    /// Log a session compaction event to the audit ledger.
    ///
    /// Records the trigger reason, full compaction outcome metrics, and
    /// optional correlation ID for tracing.
    pub async fn log_session_compaction(
        &self,
        session_id: Uuid,
        agent_id: Uuid,
        trigger: crate::session::CompactionTrigger,
        outcome: &crate::session::CompactionOutcome,
        correlation_id: Option<Uuid>,
    ) -> Result<i64> {
        self.append_event(
            Some(agent_id),
            "session.compacted",
            serde_json::json!({
                "session_id": session_id,
                "trigger": trigger.to_string(),
                "tokens_before": outcome.tokens_before,
                "tokens_after": outcome.tokens_after,
                "messages_pruned": outcome.messages_pruned,
                "messages_summarized": outcome.messages_summarized,
                "memories_flushed": outcome.memories_flushed,
                "tool_results_trimmed": outcome.tool_results_trimmed,
                "tool_results_cleared": outcome.tool_results_cleared,
                "duration_ms": outcome.duration_ms,
                "nothing_to_store": outcome.nothing_to_store,
                "flush_failed": outcome.flush_failed,
            }),
            correlation_id,
            None, // session.compacted is not a privileged action
            None,
            None,
            None,
        )
        .await
    }

    /// Log a database migration to the audit ledger.
    pub async fn log_migration(
        &self,
        migration_version: &str,
        owner_signing_key: Option<&SigningKey>,
    ) -> Result<i64> {
        self.append_event(
            None,
            "db.migration",
            serde_json::json!({
                "migration_version": migration_version,
            }),
            None,
            owner_signing_key,
            None,
            None,
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
            "SELECT event_id, ts, actor_id, action_type, payload_hash, prev_hash, event_hash, core_signature, correlation_id, metadata, quantum_salt FROM ledger_events WHERE actor_id = $1 ORDER BY ts DESC LIMIT $2",
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
            "SELECT event_id, ts, actor_id, action_type, payload_hash, prev_hash, event_hash, core_signature, correlation_id, metadata, quantum_salt FROM ledger_events WHERE action_type = $1 ORDER BY ts DESC LIMIT $2",
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
            "SELECT event_id, ts, actor_id, action_type, payload_hash, prev_hash, event_hash, core_signature, correlation_id, metadata, quantum_salt FROM ledger_events WHERE correlation_id = $1 ORDER BY ts ASC",
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
            "SELECT event_id, ts, actor_id, action_type, payload_hash, prev_hash, event_hash, core_signature, correlation_id, metadata, quantum_salt FROM ledger_events ORDER BY ts DESC LIMIT $1",
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(rows)
    }

    // ── Anchor Methods ────────────────────────────────────────────────────

    /// Publish a ledger slice anchor to a chain anchor service.
    ///
    /// Computes a blake3 merkle root over all event hashes in the slice
    /// and stores the anchor for external verification.
    ///
    /// # Arguments
    /// - `from_event_id`: Starting event ID (inclusive)
    /// - `to_event_id`: Ending event ID (inclusive)
    /// - `chain_anchor`: Implementation of ChainAnchor trait
    /// - `owner_signing_key`: Optional key to sign the anchor event
    ///
    /// # Returns
    /// - The anchor ID (UUID) of the stored anchor
    pub async fn publish_ledger_anchor(
        &self,
        from_event_id: i64,
        to_event_id: i64,
        chain_anchor: &dyn ChainAnchor,
        owner_signing_key: Option<&SigningKey>,
    ) -> Result<String> {
        // Fetch events in the slice
        let rows = sqlx::query(
            "SELECT event_id, event_hash FROM ledger_events WHERE event_id >= $1 AND event_id <= $2 ORDER BY event_id ASC"
        )
        .bind(from_event_id)
        .bind(to_event_id)
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        if rows.is_empty() {
            return Err(Error::Validation(format!(
                "No ledger events found in range {} to {}",
                from_event_id, to_event_id
            )));
        }

        // Compute merkle root over event hashes
        let mut hashes: Vec<[u8; 32]> = Vec::new();
        for row in &rows {
            let event_hash: String = row.try_get("event_hash").map_err(|e| {
                Error::DatabaseMessage(format!("Failed to extract event_hash: {}", e))
            })?;
            let hash_bytes = hex::decode(&event_hash)
                .map_err(|e| Error::Validation(format!("Invalid event_hash hex: {}", e)))?;
            if hash_bytes.len() != 32 {
                return Err(Error::Validation(format!(
                    "Event hash has wrong length: expected 32, got {}",
                    hash_bytes.len()
                )));
            }
            let mut arr = [0u8; 32];
            arr.copy_from_slice(&hash_bytes);
            hashes.push(arr);
        }

        // Compute merkle root using blake3 pairwise hashing
        let root_hash = Self::compute_merkle_root(&hashes);
        let root_hash_hex = hex::encode(root_hash);

        // Prepare metadata
        let metadata = serde_json::json!({
            "ledger_event_from": from_event_id,
            "ledger_event_to": to_event_id,
            "event_count": rows.len(),
            "merkle_root": root_hash_hex,
        });

        // Store anchor
        let anchor_id = chain_anchor
            .anchor_hash(&root_hash_hex, metadata.clone())
            .await?;

        // Log anchor event to ledger
        self.log_anchor_published(
            from_event_id,
            to_event_id,
            &anchor_id,
            &root_hash_hex,
            i64::try_from(rows.len()).unwrap_or(i64::MAX),
            owner_signing_key,
        )
        .await?;

        tracing::info!(
            anchor_id = %anchor_id,
            from_event = from_event_id,
            to_event = to_event_id,
            event_count = rows.len(),
            merkle_root = %root_hash_hex,
            "Ledger slice anchored"
        );

        Ok(anchor_id)
    }

    /// Compute a merkle root from a list of blake3 hashes.
    fn compute_merkle_root(hashes: &[[u8; 32]]) -> [u8; 32] {
        if hashes.is_empty() {
            return blake3::hash(b"").into();
        }
        if hashes.len() == 1 {
            return hashes[0];
        }

        let mut level: Vec<[u8; 32]> = hashes.to_vec();
        while level.len() > 1 {
            let mut next_level: Vec<[u8; 32]> = Vec::new();
            for i in (0..level.len()).step_by(2) {
                let left = level[i];
                let right = if i + 1 < level.len() {
                    level[i + 1]
                } else {
                    left // Duplicate last element if odd
                };
                let combined = [&left[..], &right[..]].concat();
                let hash = blake3::hash(&combined);
                next_level.push(hash.into());
            }
            level = next_level;
        }
        level[0]
    }

    /// Log an anchor publication event to the ledger.
    async fn log_anchor_published(
        &self,
        from_event_id: i64,
        to_event_id: i64,
        anchor_id: &str,
        merkle_root: &str,
        event_count: i64,
        owner_signing_key: Option<&SigningKey>,
    ) -> Result<i64> {
        self.append_event(
            None,
            "ledger.anchor_published",
            serde_json::json!({
                "from_event_id": from_event_id,
                "to_event_id": to_event_id,
                "anchor_id": anchor_id,
                "merkle_root": merkle_root,
                "event_count": event_count,
            }),
            None,
            owner_signing_key,
            None,
            None,
            None,
        )
        .await
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

        let base_hash = Ledger::compute_event_hash(&ts, actor, action, payload_hash, prev_hash, None);

        // Changing any field should produce a different hash
        let different_action =
            Ledger::compute_event_hash(&ts, actor, "other.action", payload_hash, prev_hash, None);
        assert_ne!(base_hash, different_action);

        let different_actor =
            Ledger::compute_event_hash(&ts, Some(Uuid::new_v4()), action, payload_hash, prev_hash, None);
        assert_ne!(base_hash, different_actor);

        let different_prev =
            Ledger::compute_event_hash(&ts, actor, action, payload_hash, Some("other"), None);
        assert_ne!(base_hash, different_prev);

        let no_prev = Ledger::compute_event_hash(&ts, actor, action, payload_hash, None, None);
        assert_ne!(base_hash, no_prev);
    }

    #[test]
    fn test_compute_event_hash_genesis() {
        let ts = Utc::now();
        let hash = Ledger::compute_event_hash(&ts, None, "genesis", "empty", None, None);
        assert_eq!(hash.len(), 64);
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_append_event_updates_last_hash() {
        let db_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://carnelian:carnelian@localhost:5432/carnelian".into());
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .expect("Failed to connect to database");

        let ledger = Ledger::new(pool);
        ledger
            .load_last_hash()
            .await
            .expect("load_last_hash failed");

        let event_id = ledger
            .append_event(
                None,
                "test.append",
                serde_json::json!({"test": true}),
                None,
                None,
                None,
                None,
                None,
            )
            .await
            .expect("append_event failed");

        assert!(event_id > 0);
        let last = ledger.last_hash.read().await.clone();
        assert!(last.is_some(), "last_hash should be set after append");
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_verify_chain_empty_ledger() {
        let db_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://carnelian:carnelian@localhost:5432/carnelian".into());
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .expect("Failed to connect to database");

        // Clean slate: remove any events left by other tests
        sqlx::query("TRUNCATE ledger_events")
            .execute(&pool)
            .await
            .expect("Failed to truncate ledger_events");

        let ledger = Ledger::new(pool);
        let result = ledger
            .verify_chain(None)
            .await
            .expect("verify_chain failed");
        assert!(result, "Chain should verify (empty or valid)");
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_verify_chain_multiple_events() {
        let db_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://carnelian:carnelian@localhost:5432/carnelian".into());
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .expect("Failed to connect to database");

        // Clean slate: remove any events left by other tests
        sqlx::query("TRUNCATE ledger_events")
            .execute(&pool)
            .await
            .expect("Failed to truncate ledger_events");

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
                    None,
                    None,
                    None,
                    None,
                )
                .await
                .expect("append_event failed");
        }

        let result = ledger
            .verify_chain(None)
            .await
            .expect("verify_chain failed");
        assert!(result, "Chain should verify after multiple appends");
    }

    #[test]
    fn test_is_privileged_action() {
        assert!(is_privileged_action("capability.grant"));
        assert!(is_privileged_action("capability.revoke"));
        assert!(is_privileged_action("config.change"));
        assert!(is_privileged_action("db.migration"));
        assert!(is_privileged_action("approval.granted"));
        assert!(is_privileged_action("approval.denied"));
        assert!(is_privileged_action("safe_mode.enabled"));
        assert!(is_privileged_action("safe_mode.disabled"));

        assert!(!is_privileged_action("test.action"));
        assert!(!is_privileged_action("session.compacted"));
        assert!(!is_privileged_action("heartbeat.tick"));
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_append_privileged_event_with_signature() {
        let db_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://carnelian:carnelian@localhost:5432/carnelian".into());
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .expect("Failed to connect to database");

        sqlx::query("TRUNCATE ledger_events")
            .execute(&pool)
            .await
            .expect("Failed to truncate ledger_events");

        let (signing_key, _) = crate::crypto::generate_ed25519_keypair();
        let ledger = Ledger::new(pool.clone());
        ledger
            .load_last_hash()
            .await
            .expect("load_last_hash failed");

        // Privileged action with signing key should produce a signature
        let event_id = ledger
            .append_event(
                None,
                "capability.grant",
                serde_json::json!({"test": true}),
                None,
                Some(&signing_key),
                None,
                None,
                None,
            )
            .await
            .expect("append_event failed");

        let event: LedgerEvent = sqlx::query_as(
            "SELECT event_id, ts, actor_id, action_type, payload_hash, prev_hash, event_hash, core_signature, correlation_id, metadata, quantum_salt FROM ledger_events WHERE event_id = $1",
        )
        .bind(event_id)
        .fetch_one(&pool)
        .await
        .expect("Failed to fetch event");

        assert!(
            event.core_signature.is_some(),
            "Privileged event should have signature"
        );
        assert_eq!(
            event.core_signature.as_ref().unwrap().len(),
            128,
            "Hex signature should be 128 chars"
        );
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_verify_chain_with_signatures() {
        let db_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://carnelian:carnelian@localhost:5432/carnelian".into());
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .expect("Failed to connect to database");

        sqlx::query("TRUNCATE ledger_events")
            .execute(&pool)
            .await
            .expect("Failed to truncate ledger_events");

        let (signing_key, _) = crate::crypto::generate_ed25519_keypair();
        let public_key_hex = crate::crypto::public_key_from_signing_key(&signing_key);

        let ledger = Ledger::new(pool.clone());
        ledger
            .load_last_hash()
            .await
            .expect("load_last_hash failed");

        // Mix of signed privileged and unsigned non-privileged events
        ledger
            .append_event(None, "test.action", serde_json::json!({}), None, None, None, None, None)
            .await
            .expect("append failed");
        ledger
            .append_event(
                None,
                "capability.grant",
                serde_json::json!({"g": 1}),
                None,
                Some(&signing_key),
                None,
                None,
                None,
            )
            .await
            .expect("append failed");
        ledger
            .append_event(
                None,
                "config.change",
                serde_json::json!({"k": "v"}),
                None,
                Some(&signing_key),
                None,
                None,
                None,
            )
            .await
            .expect("append failed");
        ledger
            .append_event(None, "test.other", serde_json::json!({}), None, None, None, None, None)
            .await
            .expect("append failed");

        let result = ledger
            .verify_chain(Some(&public_key_hex))
            .await
            .expect("verify_chain failed");
        assert!(result, "Chain with valid signatures should verify");
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_verify_chain_rejects_invalid_signature() {
        let db_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://carnelian:carnelian@localhost:5432/carnelian".into());
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect(&db_url)
            .await
            .expect("Failed to connect to database");

        sqlx::query("TRUNCATE ledger_events")
            .execute(&pool)
            .await
            .expect("Failed to truncate ledger_events");

        let (signing_key, _) = crate::crypto::generate_ed25519_keypair();
        let public_key_hex = crate::crypto::public_key_from_signing_key(&signing_key);

        let ledger = Ledger::new(pool.clone());
        ledger
            .load_last_hash()
            .await
            .expect("load_last_hash failed");

        // Append a signed privileged event
        let event_id = ledger
            .append_event(
                None,
                "capability.grant",
                serde_json::json!({"test": true}),
                None,
                Some(&signing_key),
                None,
                None,
                None,
            )
            .await
            .expect("append failed");

        // Tamper with the signature in the database
        sqlx::query("UPDATE ledger_events SET core_signature = $1 WHERE event_id = $2")
            .bind("00".repeat(64)) // 128 hex chars = 64 bytes, but wrong signature
            .bind(event_id)
            .execute(&pool)
            .await
            .expect("Failed to tamper signature");

        let result = ledger
            .verify_chain(Some(&public_key_hex))
            .await
            .expect("verify_chain failed");
        assert!(
            !result,
            "Chain with tampered signature should fail verification"
        );
    }
}
