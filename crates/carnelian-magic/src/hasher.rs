use crate::entropy::MixedEntropyProvider;
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

    pub fn with_os_entropy() -> Self {
        let node_id = uuid::Uuid::new_v4();
        let mixed_provider = MixedEntropyProvider::new(None, None, None, node_id);
        Self {
            entropy: Arc::new(mixed_provider),
        }
    }

    fn derive_salt(
        row_id: Uuid,
        table: &str,
        creation_timestamp: DateTime<Utc>,
    ) -> Vec<u8> {
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

    pub async fn compute(
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

    pub fn verify(
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

    pub async fn batch_compute(
        &self,
        rows: Vec<(Uuid, Vec<u8>, DateTime<Utc>)>,
        table: &str,
    ) -> Vec<(Uuid, String)> {
        let mut results = Vec::new();
        
        for (row_id, content, created_at) in rows {
            match self.compute(table, row_id, &content, created_at).await {
                Ok(checksum) => results.push((row_id, checksum)),
                Err(e) => {
                    tracing::warn!("Failed to compute checksum for row {}: {}", row_id, e);
                }
            }
        }
        
        results
    }
}
