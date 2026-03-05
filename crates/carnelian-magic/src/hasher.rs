use crate::entropy::MixedEntropyProvider;
use crate::error::Result;
use blake3;
use chrono::{DateTime, Utc};
use hex;
use std::sync::Arc;
use uuid::Uuid;

pub struct QuantumHasher {
    _entropy: Option<Arc<MixedEntropyProvider>>,
}

impl QuantumHasher {
    pub fn new(entropy: Arc<MixedEntropyProvider>) -> Self {
        Self {
            _entropy: Some(entropy),
        }
    }

    pub fn with_os_entropy() -> Self {
        Self { _entropy: None }
    }

    fn derive_salt(row_id: Uuid, table: &str, creation_timestamp: DateTime<Utc>) -> Vec<u8> {
        let mut hasher = blake3::Hasher::new_derive_key("carnelian-quantum-salt-v1");
        hasher.update(row_id.as_bytes());
        hasher.update(table.as_bytes());

        let timestamp_nanos = creation_timestamp.timestamp_nanos_opt().unwrap_or(0);
        hasher.update(&timestamp_nanos.to_le_bytes());

        hasher.finalize().as_bytes().to_vec()
    }

    fn checksum_bytes(content: &[u8], salt: &[u8], table: &str, row_id: Uuid) -> String {
        let mut hasher = blake3::Hasher::new_derive_key("carnelian-quantum-checksum-v1");
        hasher.update(content);
        hasher.update(salt);
        hasher.update(table.as_bytes());
        hasher.update(row_id.as_bytes());

        let hash = hasher.finalize();
        hex::encode(hash.as_bytes())
    }

    /// Compute checksum with explicit timestamp (for DB verification with fetched timestamps)
    pub fn compute_with_ts(
        &self,
        table: &str,
        row_id: Uuid,
        content: &[u8],
        created_at: DateTime<Utc>,
    ) -> Result<String> {
        let salt = Self::derive_salt(row_id, table, created_at);
        let checksum = Self::checksum_bytes(content, &salt, table, row_id);

        Ok(checksum)
    }

    /// Verify checksum with explicit timestamp (for DB verification with fetched timestamps)
    pub fn verify_with_ts(
        &self,
        table: &str,
        row_id: Uuid,
        content: &[u8],
        created_at: DateTime<Utc>,
        stored: &str,
    ) -> bool {
        let salt = Self::derive_salt(row_id, table, created_at);
        let recomputed = Self::checksum_bytes(content, &salt, table, row_id);

        recomputed == stored
    }

    /// Compute checksum using current timestamp (contract-compliant API)
    ///
    /// **Note:** This uses `Utc::now()` for the timestamp. Only suitable for immediate
    /// post-compute verification checks. For DB-backed verification, use `compute_with_ts`
    /// with the row's actual creation timestamp.
    pub fn compute(&self, table: &str, row_id: Uuid, content: &[u8]) -> Result<String> {
        self.compute_with_ts(table, row_id, content, Utc::now())
    }

    /// Verify checksum using current timestamp (contract-compliant API)
    ///
    /// **Note:** This uses `Utc::now()` for the timestamp. Only suitable for immediate
    /// post-compute verification checks. For DB-backed verification, use `verify_with_ts`
    /// with the row's actual creation timestamp.
    pub fn verify(&self, table: &str, row_id: Uuid, content: &[u8], stored: &str) -> bool {
        self.verify_with_ts(table, row_id, content, Utc::now(), stored)
    }

    /// Batch compute checksums using current timestamp (contract-compliant API)
    ///
    /// **Note:** This uses `Utc::now()` for all rows. Only suitable for immediate
    /// batch operations. For DB-backed verification, use `compute_with_ts` per row
    /// with actual creation timestamps.
    pub fn batch_compute(&self, rows: Vec<(Uuid, Vec<u8>)>, table: &str) -> Vec<(Uuid, String)> {
        let mut results = Vec::new();
        let now = Utc::now();

        for (row_id, content) in rows {
            match self.compute_with_ts(table, row_id, &content, now) {
                Ok(checksum) => results.push((row_id, checksum)),
                Err(e) => {
                    tracing::warn!("Failed to compute checksum for row {}: {}", row_id, e);
                }
            }
        }

        results
    }
}
