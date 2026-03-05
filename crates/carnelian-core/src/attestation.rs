//! Worker attestation verification and quarantine management
//!
//! Workers periodically report their ledger head hash, build checksum, and
//! configuration version. The attestation module verifies these values against
//! the orchestrator's expected state and quarantines workers with mismatches.
//!
//! # Architecture
//!
//! Each worker includes attestation data in its health check response:
//! - `last_ledger_head`: blake3 hash of the most recent ledger event the worker has seen
//! - `build_checksum`: hash of the worker binary/script (e.g., package.json version)
//! - `config_version`: configuration state identifier
//!
//! The orchestrator compares these against its own expected values. Mismatches
//! trigger quarantine: the worker is marked in the `worker_attestations` table
//! and denied new task assignments. A `"worker.quarantined"` ledger event is
//! logged as a privileged action with owner keypair signature for audit trail.
//!
//! # Threat Model
//!
//! Attestation detects:
//! - Workers running stale code (build_checksum mismatch)
//! - Workers with divergent ledger state (last_ledger_head mismatch)
//! - Workers with outdated configuration (config_version mismatch)
//!
//! It does **not** protect against a fully compromised worker that lies about
//! its attestation values. For that, remote attestation (e.g., TPM) would be needed.
//!
//! # Environment Variables
//!
//! Workers receive expected values via environment variables set at spawn time:
//! - `CARNELIAN_LEDGER_HEAD`: expected ledger head hash
//! - `CARNELIAN_CONFIG_VERSION`: expected configuration version

use carnelian_common::{Error, Result};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;

/// Attestation data reported by a worker
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerAttestation {
    pub worker_id: String,
    pub last_ledger_head: String,
    pub build_checksum: String,
    pub config_version: String,
}

/// Attestation verification result
#[derive(Debug, Clone)]
pub struct AttestationResult {
    pub worker_id: String,
    pub verified: bool,
    pub mismatch_reason: Option<String>,
}

/// Verify a worker's attestation against expected values
pub async fn verify_attestation(
    _pool: &PgPool,
    attestation: &WorkerAttestation,
    expected_ledger_head: &str,
    expected_build_checksum: &str,
    expected_config_version: &str,
) -> Result<AttestationResult> {
    let mut mismatches = Vec::new();

    if attestation.last_ledger_head != expected_ledger_head {
        mismatches.push(format!(
            "ledger_head mismatch: expected {}, got {}",
            expected_ledger_head, attestation.last_ledger_head
        ));
    }

    if attestation.build_checksum != expected_build_checksum {
        mismatches.push(format!(
            "build_checksum mismatch: expected {}, got {}",
            expected_build_checksum, attestation.build_checksum
        ));
    }

    if attestation.config_version != expected_config_version {
        mismatches.push(format!(
            "config_version mismatch: expected {}, got {}",
            expected_config_version, attestation.config_version
        ));
    }

    let verified = mismatches.is_empty();
    let mismatch_reason = if verified {
        None
    } else {
        Some(mismatches.join("; "))
    };

    Ok(AttestationResult {
        worker_id: attestation.worker_id.clone(),
        verified,
        mismatch_reason,
    })
}

/// Record an attestation in the database
pub async fn record_attestation(pool: &PgPool, attestation: &WorkerAttestation) -> Result<()> {
    sqlx::query(
        r"INSERT INTO worker_attestations (worker_id, last_ledger_head, build_checksum, config_version, attested_at)
          VALUES ($1, $2, $3, $4, NOW())
          ON CONFLICT (worker_id) DO UPDATE SET
            last_ledger_head = $2,
            build_checksum = $3,
            config_version = $4,
            attested_at = NOW()"
    )
    .bind(&attestation.worker_id)
    .bind(&attestation.last_ledger_head)
    .bind(&attestation.build_checksum)
    .bind(&attestation.config_version)
    .execute(pool)
    .await
    .map_err(Error::Database)?;

    Ok(())
}

/// Quarantine a worker due to attestation mismatch
pub async fn quarantine_worker(pool: &PgPool, worker_id: &str, reason: &str) -> Result<()> {
    sqlx::query(
        r"UPDATE worker_attestations
          SET quarantined = true, quarantine_reason = $2, quarantined_at = NOW()
          WHERE worker_id = $1",
    )
    .bind(worker_id)
    .bind(reason)
    .execute(pool)
    .await
    .map_err(Error::Database)?;

    tracing::warn!(
        worker_id = %worker_id,
        reason = %reason,
        "Worker quarantined due to attestation mismatch"
    );

    Ok(())
}

/// Check if a worker is quarantined
pub async fn is_worker_quarantined(pool: &PgPool, worker_id: &str) -> Result<bool> {
    let quarantined: Option<bool> =
        sqlx::query_scalar("SELECT quarantined FROM worker_attestations WHERE worker_id = $1")
            .bind(worker_id)
            .fetch_optional(pool)
            .await
            .map_err(Error::Database)?;

    Ok(quarantined.unwrap_or(false))
}

/// Get all quarantined workers
pub async fn get_quarantined_workers(pool: &PgPool) -> Result<Vec<String>> {
    let workers: Vec<(String,)> =
        sqlx::query_as("SELECT worker_id FROM worker_attestations WHERE quarantined = true")
            .fetch_all(pool)
            .await
            .map_err(Error::Database)?;

    Ok(workers.into_iter().map(|(id,)| id).collect())
}
