//! Chain anchor implementation for local database anchoring
//!
//! Provides a concrete implementation of the ChainAnchor trait that stores
//! ledger slice hashes in the local database, enabling verification and
//! cross-instance proof material.

use std::collections::HashMap;
use sqlx::{PgPool, Row};
use serde_json::Value;
use uuid::Uuid;

use crate::memory::ChainAnchor;
use crate::{Error, Result};

/// A chain anchor backed by the local PostgreSQL database.
/// Stores hash anchors for ledger event slices, enabling verification
/// and cross-instance proof material.
#[derive(Debug, Clone)]
pub struct LocalDbChainAnchor {
    pool: PgPool,
}

impl LocalDbChainAnchor {
    /// Create a new LocalDbChainAnchor with the given database pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get the database pool.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}

#[async_trait::async_trait]
impl ChainAnchor for LocalDbChainAnchor {
    /// Store a hash anchor in the database.
    ///
    /// # Arguments
    /// - `hash`: The hash to anchor (typically a blake3 merkle root)
    /// - `metadata`: JSON metadata about the anchor (e.g., ledger event range)
    ///
    /// # Returns
    /// - `anchor_id`: The UUID of the stored anchor as a string
    async fn anchor_hash(&self, hash: &str, metadata: Value) -> Result<String> {
        // Extract ledger event range from metadata if present
        let from_event: i64 = metadata
            .get("ledger_event_from")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);
        let to_event: i64 = metadata
            .get("ledger_event_to")
            .and_then(|v| v.as_i64())
            .unwrap_or(0);

        let row = sqlx::query(
            r#"
            INSERT INTO chain_anchors (hash, ledger_event_from, ledger_event_to, metadata)
            VALUES ($1, $2, $3, $4)
            RETURNING anchor_id
            "#
        )
        .bind(hash)
        .bind(from_event)
        .bind(to_event)
        .bind(metadata)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Error::DatabaseMessage(format!("Failed to store chain anchor: {}", e)))?;

        let anchor_id: Uuid = row.try_get("anchor_id")
            .map_err(|e| Error::DatabaseMessage(format!("Failed to extract anchor_id: {}", e)))?;

        tracing::info!(
            anchor_id = %anchor_id,
            hash = %hash,
            from_event,
            to_event,
            "Ledger slice anchored"
        );

        Ok(anchor_id.to_string())
    }

    /// Verify that a stored anchor matches the provided hash.
    ///
    /// # Arguments
    /// - `anchor_id`: The anchor UUID to verify
    /// - `hash`: The expected hash value
    ///
    /// # Returns
    /// - `true` if the stored hash matches the provided hash
    /// - `false` if no anchor found or hash mismatch
    async fn verify_anchor(&self, anchor_id: &str, hash: &str) -> Result<bool> {
        let anchor_uuid = Uuid::parse_str(anchor_id)
            .map_err(|e| Error::Validation(format!("Invalid anchor_id UUID: {}", e)))?;

        let row = sqlx::query(
            "SELECT hash FROM chain_anchors WHERE anchor_id = $1"
        )
        .bind(anchor_uuid)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::DatabaseMessage(format!("Failed to query anchor: {}", e)))?;

        match row {
            Some(row) => {
                let stored_hash: String = row.try_get("hash")
                    .map_err(|e| Error::DatabaseMessage(format!("Failed to extract hash: {}", e)))?;
                let matches = stored_hash == hash;

                // Update verified flag
                sqlx::query(
                    "UPDATE chain_anchors SET verified = $1 WHERE anchor_id = $2"
                )
                .bind(matches)
                .bind(anchor_uuid)
                .execute(&self.pool)
                .await
                .map_err(|e| Error::DatabaseMessage(format!("Failed to update verified flag: {}", e)))?;

                tracing::info!(
                    anchor_id = %anchor_uuid,
                    matches,
                    "Anchor verification check"
                );

                Ok(matches)
            }
            None => {
                tracing::warn!(anchor_id = %anchor_uuid, "Anchor not found for verification");
                Ok(false)
            }
        }
    }

    /// Get the full proof material for an anchor.
    ///
    /// # Arguments
    /// - `anchor_id`: The anchor UUID to retrieve
    ///
    /// # Returns
    /// - JSON object with anchor_id, hash, ledger_event_from/to, published_at, metadata
    async fn get_anchor_proof(&self, anchor_id: &str) -> Result<Option<Value>> {
        let anchor_uuid = Uuid::parse_str(anchor_id)
            .map_err(|e| Error::Validation(format!("Invalid anchor_id UUID: {}", e)))?;

        let row = sqlx::query(
            r#"
            SELECT anchor_id, hash, ledger_event_from, ledger_event_to, 
                   published_at, metadata, verified
            FROM chain_anchors 
            WHERE anchor_id = $1
            "#
        )
        .bind(anchor_uuid)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::DatabaseMessage(format!("Failed to query anchor proof: {}", e)))?;

        match row {
            Some(row) => {
                let anchor_id: Uuid = row.try_get("anchor_id")
                    .map_err(|e| Error::DatabaseMessage(format!("Failed to extract anchor_id: {}", e)))?;
                let hash: String = row.try_get("hash")
                    .map_err(|e| Error::DatabaseMessage(format!("Failed to extract hash: {}", e)))?;
                let from_event: i64 = row.try_get("ledger_event_from")
                    .map_err(|e| Error::DatabaseMessage(format!("Failed to extract from_event: {}", e)))?;
                let to_event: i64 = row.try_get("ledger_event_to")
                    .map_err(|e| Error::DatabaseMessage(format!("Failed to extract to_event: {}", e)))?;
                let published_at: chrono::DateTime<chrono::Utc> = row.try_get("published_at")
                    .map_err(|e| Error::DatabaseMessage(format!("Failed to extract published_at: {}", e)))?;
                let metadata: Value = row.try_get("metadata")
                    .map_err(|e| Error::DatabaseMessage(format!("Failed to extract metadata: {}", e)))?;
                let verified: bool = row.try_get("verified")
                    .map_err(|e| Error::DatabaseMessage(format!("Failed to extract verified: {}", e)))?;

                let proof = serde_json::json!({
                    "anchor_id": anchor_id.to_string(),
                    "hash": hash,
                    "ledger_event_from": from_event,
                    "ledger_event_to": to_event,
                    "published_at": published_at.to_rfc3339(),
                    "metadata": metadata,
                    "verified": verified,
                });

                Ok(Some(proof))
            }
            None => Ok(None),
        }
    }
}

/// Query anchors by event range (for finding anchors covering a specific ledger slice).
pub async fn find_anchors_by_event_range(
    pool: &PgPool,
    from_event: i64,
    to_event: i64,
) -> Result<Vec<HashMap<String, Value>>> {
    let rows = sqlx::query(
        r#"
        SELECT anchor_id, hash, ledger_event_from, ledger_event_to, 
               published_at, metadata, verified
        FROM chain_anchors 
        WHERE ledger_event_from <= $2 AND ledger_event_to >= $1
        ORDER BY ledger_event_from ASC
        "#
    )
    .bind(to_event)
    .bind(from_event)
    .fetch_all(pool)
    .await
    .map_err(|e| Error::DatabaseMessage(format!("Failed to query anchors by range: {}", e)))?;

    let mut results = Vec::new();
    for row in rows {
        let anchor_id: Uuid = row.try_get("anchor_id")
            .map_err(|e| Error::DatabaseMessage(format!("Failed to extract anchor_id: {}", e)))?;
        let hash: String = row.try_get("hash")
            .map_err(|e| Error::DatabaseMessage(format!("Failed to extract hash: {}", e)))?;
        let from_event: i64 = row.try_get("ledger_event_from")
            .map_err(|e| Error::DatabaseMessage(format!("Failed to extract from_event: {}", e)))?;
        let to_event: i64 = row.try_get("ledger_event_to")
            .map_err(|e| Error::DatabaseMessage(format!("Failed to extract to_event: {}", e)))?;
        let published_at: chrono::DateTime<chrono::Utc> = row.try_get("published_at")
            .map_err(|e| Error::DatabaseMessage(format!("Failed to extract published_at: {}", e)))?;
        let metadata: Value = row.try_get("metadata")
            .map_err(|e| Error::DatabaseMessage(format!("Failed to extract metadata: {}", e)))?;
        let verified: bool = row.try_get("verified")
            .map_err(|e| Error::DatabaseMessage(format!("Failed to extract verified: {}", e)))?;

        let mut anchor = HashMap::new();
        anchor.insert("anchor_id".to_string(), Value::String(anchor_id.to_string()));
        anchor.insert("hash".to_string(), Value::String(hash));
        anchor.insert("ledger_event_from".to_string(), Value::Number(from_event.into()));
        anchor.insert("ledger_event_to".to_string(), Value::Number(to_event.into()));
        anchor.insert("published_at".to_string(), Value::String(published_at.to_rfc3339()));
        anchor.insert("metadata".to_string(), metadata);
        anchor.insert("verified".to_string(), Value::Bool(verified));
        results.push(anchor);
    }

    Ok(results)
}

/// List recent anchors (for discovery and sync).
pub async fn list_recent_anchors(
    pool: &PgPool,
    limit: i64,
) -> Result<Vec<HashMap<String, Value>>> {
    let rows = sqlx::query(
        r#"
        SELECT anchor_id, hash, ledger_event_from, ledger_event_to, 
               published_at, metadata, verified
        FROM chain_anchors 
        ORDER BY published_at DESC
        LIMIT $1
        "#
    )
    .bind(limit)
    .fetch_all(pool)
    .await
    .map_err(|e| Error::DatabaseMessage(format!("Failed to list anchors: {}", e)))?;

    let mut results = Vec::new();
    for row in rows {
        let anchor_id: Uuid = row.try_get("anchor_id")
            .map_err(|e| Error::DatabaseMessage(format!("Failed to extract anchor_id: {}", e)))?;
        let hash: String = row.try_get("hash")
            .map_err(|e| Error::DatabaseMessage(format!("Failed to extract hash: {}", e)))?;
        let from_event: i64 = row.try_get("ledger_event_from")
            .map_err(|e| Error::DatabaseMessage(format!("Failed to extract from_event: {}", e)))?;
        let to_event: i64 = row.try_get("ledger_event_to")
            .map_err(|e| Error::DatabaseMessage(format!("Failed to extract to_event: {}", e)))?;
        let published_at: chrono::DateTime<chrono::Utc> = row.try_get("published_at")
            .map_err(|e| Error::DatabaseMessage(format!("Failed to extract published_at: {}", e)))?;
        let metadata: Value = row.try_get("metadata")
            .map_err(|e| Error::DatabaseMessage(format!("Failed to extract metadata: {}", e)))?;
        let verified: bool = row.try_get("verified")
            .map_err(|e| Error::DatabaseMessage(format!("Failed to extract verified: {}", e)))?;

        let mut anchor = HashMap::new();
        anchor.insert("anchor_id".to_string(), Value::String(anchor_id.to_string()));
        anchor.insert("hash".to_string(), Value::String(hash));
        anchor.insert("ledger_event_from".to_string(), Value::Number(from_event.into()));
        anchor.insert("ledger_event_to".to_string(), Value::Number(to_event.into()));
        anchor.insert("published_at".to_string(), Value::String(published_at.to_rfc3339()));
        anchor.insert("metadata".to_string(), metadata);
        anchor.insert("verified".to_string(), Value::Bool(verified));
        results.push(anchor);
    }

    Ok(results)
}
