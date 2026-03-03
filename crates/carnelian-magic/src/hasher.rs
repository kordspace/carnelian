use crate::entropy::{EntropyProvider, MixedEntropyProvider, OsEntropyProvider};
use crate::error::Result;
use blake3;
use chrono::{DateTime, Utc};
use hex;
use std::sync::Arc;
use uuid::Uuid;

pub struct QuantumHasher {
    entropy: Arc<MixedEntropyProvider>,
}

impl QuantumHasher {
    pub fn new(entropy: Arc<MixedEntropyProvider>) -> Self {
        Self { entropy }
    }

    fn derive_salt(
        row_id: Uuid,
        table: &str,
        creation_timestamp: DateTime<Utc>,
        entropy_bytes: &[u8],
    ) -> Vec<u8> {
        let mut hasher = blake3::Hasher::new_derive_key("carnelian-quantum-salt-v1");
        hasher.update(row_id.as_bytes());
        hasher.update(table.as_bytes());
        
        let timestamp_nanos = creation_timestamp.timestamp_nanos_opt().unwrap_or(0);
        hasher.update(&timestamp_nanos.to_le_bytes());
        
        let mut salt = hasher.finalize().as_bytes().to_vec();
        
        for (i, &byte) in entropy_bytes.iter().take(16).enumerate() {
            salt[i] ^= byte;
        }
        
        salt
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

    pub async fn compute(
        &self,
        table: &str,
        row_id: Uuid,
        content: &[u8],
        created_at: DateTime<Utc>,
    ) -> Result<String> {
        let entropy_bytes = match self.entropy.get_bytes(16).await {
            Ok(bytes) => bytes,
            Err(e) => {
                tracing::warn!("Quantum entropy unavailable, falling back to OS: {}", e);
                OsEntropyProvider::new().get_bytes(16).await?
            }
        };

        let salt = Self::derive_salt(row_id, table, created_at, &entropy_bytes);
        let checksum = Self::checksum_bytes(content, &salt, table, row_id);
        
        Ok(checksum)
    }

    pub fn verify(
        &self,
        table: &str,
        row_id: Uuid,
        content: &[u8],
        created_at: DateTime<Utc>,
        stored: &str,
    ) -> bool {
        let mut hasher = blake3::Hasher::new_derive_key("carnelian-quantum-salt-v1");
        hasher.update(row_id.as_bytes());
        hasher.update(table.as_bytes());
        
        let timestamp_nanos = created_at.timestamp_nanos_opt().unwrap_or(0);
        hasher.update(&timestamp_nanos.to_le_bytes());
        
        let deterministic_salt = hasher.finalize().as_bytes().to_vec();
        
        let recomputed = Self::checksum_bytes(content, &deterministic_salt, table, row_id);
        
        recomputed == stored
    }

    pub async fn batch_compute(
        &self,
        rows: Vec<(Uuid, Vec<u8>, DateTime<Utc>)>,
        table: &str,
    ) -> Vec<(Uuid, Result<String>)> {
        let mut results = Vec::new();
        
        for (row_id, content, created_at) in rows {
            let result = self.compute(table, row_id, &content, created_at).await;
            results.push((row_id, result));
        }
        
        results
    }
}
