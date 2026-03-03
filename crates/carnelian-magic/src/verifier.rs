use crate::error::{MagicError, Result};
use crate::hasher::QuantumHasher;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum RowIdentifier {
    Uuid(Uuid),
    I64(i64),
}

impl RowIdentifier {
    fn to_uuid(&self) -> Uuid {
        match self {
            RowIdentifier::Uuid(uuid) => *uuid,
            RowIdentifier::I64(id) => Uuid::from_u128(*id as u128),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TamperedRow {
    pub row_id: RowIdentifier,
    pub table: String,
    pub expected_checksum: String,
    pub stored_checksum: String,
    pub detected_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerificationReport {
    pub table: String,
    pub total_rows: i64,
    pub verified_rows: i64,
    pub missing_checksum_rows: i64,
    pub tampered_rows: Vec<TamperedRow>,
    pub overall_status: VerificationStatus,
    pub verified_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum VerificationStatus {
    Ok,
    Tampered,
    Partial,
}

pub struct QuantumIntegrityVerifier {
    hasher: QuantumHasher,
}

impl QuantumIntegrityVerifier {
    pub fn new(hasher: QuantumHasher) -> Self {
        Self { hasher }
    }

    fn table_query(table: &str) -> Result<(&'static str, &'static str, &'static str)> {
        match table {
            "memories" => Ok((
                "SELECT memory_id, content, quantum_checksum, created_at FROM memories",
                "memory_id",
                "content",
            )),
            "session_messages" => Ok((
                "SELECT message_id, content, quantum_checksum, ts FROM session_messages",
                "message_id",
                "content",
            )),
            "elixirs" => Ok((
                "SELECT elixir_id, dataset::text, quantum_checksum, created_at FROM elixirs",
                "elixir_id",
                "dataset",
            )),
            "task_runs" => Ok((
                "SELECT run_id, COALESCE(error, ''), quantum_checksum, started_at FROM task_runs",
                "run_id",
                "error",
            )),
            _ => Err(MagicError::ProviderError {
                provider: "verifier".to_string(),
                message: format!("Unknown table: {}", table),
            }),
        }
    }

    pub async fn verify_table(&self, table: &str, pool: &PgPool) -> Result<VerificationReport> {
        let (select_sql, _pk_column, _content_column) = Self::table_query(table)?;

        let rows: Vec<(Uuid, Vec<u8>, Option<String>, DateTime<Utc>)> = match table {
            "memories" => sqlx::query_as::<_, (Uuid, Vec<u8>, Option<String>, DateTime<Utc>)>(
                select_sql,
            )
            .fetch_all(pool)
            .await?,
            "session_messages" => {
                let rows_i64: Vec<(i64, String, Option<String>, DateTime<Utc>)> =
                    sqlx::query_as::<_, (i64, String, Option<String>, DateTime<Utc>)>(select_sql)
                        .fetch_all(pool)
                        .await?;
                rows_i64
                    .into_iter()
                    .map(|(id, content, checksum, created_at)| {
                        (Uuid::from_u128(id as u128), content.into_bytes(), checksum, created_at)
                    })
                    .collect()
            }
            "elixirs" => {
                let rows_str: Vec<(Uuid, String, Option<String>, DateTime<Utc>)> =
                    sqlx::query_as::<_, (Uuid, String, Option<String>, DateTime<Utc>)>(select_sql)
                        .fetch_all(pool)
                        .await?;
                rows_str
                    .into_iter()
                    .map(|(id, content, checksum, created_at)| {
                        (id, content.into_bytes(), checksum, created_at)
                    })
                    .collect()
            }
            "task_runs" => {
                let rows_str: Vec<(Uuid, String, Option<String>, DateTime<Utc>)> =
                    sqlx::query_as::<_, (Uuid, String, Option<String>, DateTime<Utc>)>(select_sql)
                        .fetch_all(pool)
                        .await?;
                rows_str
                    .into_iter()
                    .map(|(id, content, checksum, created_at)| {
                        (id, content.into_bytes(), checksum, created_at)
                    })
                    .collect()
            }
            _ => unreachable!(),
        };

        let total_rows = rows.len() as i64;
        let mut verified_rows = 0i64;
        let mut missing_checksum_rows = 0i64;
        let mut tampered_rows = Vec::new();

        for (row_id, content, quantum_checksum, created_at) in rows {
            if let Some(stored_checksum) = quantum_checksum {
                let is_valid = self.hasher.verify(
                    table,
                    row_id,
                    &content,
                    created_at,
                    &stored_checksum,
                );

                if is_valid {
                    verified_rows += 1;
                } else {
                    let mut hasher =
                        blake3::Hasher::new_derive_key("carnelian-quantum-salt-v1");
                    hasher.update(row_id.as_bytes());
                    hasher.update(table.as_bytes());
                    let timestamp_nanos = created_at.timestamp_nanos_opt().unwrap_or(0);
                    hasher.update(&timestamp_nanos.to_le_bytes());
                    let deterministic_salt = hasher.finalize().as_bytes().to_vec();

                    let mut checksum_hasher =
                        blake3::Hasher::new_derive_key("carnelian-quantum-checksum-v1");
                    checksum_hasher.update(&content);
                    checksum_hasher.update(&deterministic_salt);
                    checksum_hasher.update(table.as_bytes());
                    checksum_hasher.update(row_id.as_bytes());
                    let expected_checksum = hex::encode(checksum_hasher.finalize().as_bytes());

                    let row_identifier = if table == "session_messages" {
                        RowIdentifier::I64(row_id.as_u128() as i64)
                    } else {
                        RowIdentifier::Uuid(row_id)
                    };
                    
                    tampered_rows.push(TamperedRow {
                        row_id: row_identifier,
                        table: table.to_string(),
                        expected_checksum,
                        stored_checksum,
                        detected_at: Utc::now(),
                    });
                }
            } else {
                missing_checksum_rows += 1;
            }
        }

        let overall_status = if !tampered_rows.is_empty() {
            VerificationStatus::Tampered
        } else if missing_checksum_rows > 0 {
            VerificationStatus::Partial
        } else {
            VerificationStatus::Ok
        };

        Ok(VerificationReport {
            table: table.to_string(),
            total_rows,
            verified_rows,
            missing_checksum_rows,
            tampered_rows,
            overall_status,
            verified_at: Utc::now(),
        })
    }

    pub async fn verify_row(
        &self,
        table: &str,
        row_id: RowIdentifier,
        pool: &PgPool,
    ) -> Result<Option<TamperedRow>> {
        let (select_sql, _pk_column, _content_column) = Self::table_query(table)?;
        let uuid_id = row_id.to_uuid();

        let row: Option<(Uuid, Vec<u8>, Option<String>, DateTime<Utc>)> = match table {
            "memories" => sqlx::query_as::<_, (Uuid, Vec<u8>, Option<String>, DateTime<Utc>)>(
                &format!("{} WHERE memory_id = $1", select_sql),
            )
            .bind(uuid_id)
            .fetch_optional(pool)
            .await?,
            "session_messages" => {
                let row_i64: Option<(i64, String, Option<String>, DateTime<Utc>)> =
                    sqlx::query_as(&format!("{} WHERE message_id = $1", select_sql))
                        .bind(uuid_id.as_u128() as i64)
                        .fetch_optional(pool)
                        .await?;
                row_i64.map(|(id, content, checksum, created_at)| {
                    (Uuid::from_u128(id as u128), content.into_bytes(), checksum, created_at)
                })
            }
            "elixirs" => {
                let row_str: Option<(Uuid, String, Option<String>, DateTime<Utc>)> =
                    sqlx::query_as(&format!("{} WHERE elixir_id = $1", select_sql))
                        .bind(uuid_id)
                        .fetch_optional(pool)
                        .await?;
                row_str.map(|(id, content, checksum, created_at)| {
                    (id, content.into_bytes(), checksum, created_at)
                })
            }
            "task_runs" => {
                let row_str: Option<(Uuid, String, Option<String>, DateTime<Utc>)> =
                    sqlx::query_as(&format!("{} WHERE run_id = $1", select_sql))
                        .bind(uuid_id)
                        .fetch_optional(pool)
                        .await?;
                row_str.map(|(id, content, checksum, created_at)| {
                    (id, content.into_bytes(), checksum, created_at)
                })
            }
            _ => return Err(MagicError::ProviderError {
                provider: "verifier".to_string(),
                message: format!("Unknown table: {}", table),
            }),
        };

        if let Some((row_id, content, quantum_checksum, created_at)) = row {
            if let Some(stored_checksum) = quantum_checksum {
                let is_valid = self.hasher.verify(
                    table,
                    row_id,
                    &content,
                    created_at,
                    &stored_checksum,
                );

                if !is_valid {
                    let mut hasher =
                        blake3::Hasher::new_derive_key("carnelian-quantum-salt-v1");
                    hasher.update(row_id.as_bytes());
                    hasher.update(table.as_bytes());
                    let timestamp_nanos = created_at.timestamp_nanos_opt().unwrap_or(0);
                    hasher.update(&timestamp_nanos.to_le_bytes());
                    let deterministic_salt = hasher.finalize().as_bytes().to_vec();

                    let mut checksum_hasher =
                        blake3::Hasher::new_derive_key("carnelian-quantum-checksum-v1");
                    checksum_hasher.update(&content);
                    checksum_hasher.update(&deterministic_salt);
                    checksum_hasher.update(table.as_bytes());
                    checksum_hasher.update(row_id.as_bytes());
                    let expected_checksum = hex::encode(checksum_hasher.finalize().as_bytes());

                    let row_identifier = if table == "session_messages" {
                        RowIdentifier::I64(row_id.as_u128() as i64)
                    } else {
                        RowIdentifier::Uuid(row_id)
                    };

                    return Ok(Some(TamperedRow {
                        row_id: row_identifier,
                        table: table.to_string(),
                        expected_checksum,
                        stored_checksum,
                        detected_at: Utc::now(),
                    }));
                }
            }
        }

        Ok(None)
    }

    pub async fn backfill_missing(&self, table: &str, pool: &PgPool) -> Result<u64> {
        let (select_sql, pk_column, _content_column) = Self::table_query(table)?;

        let query = format!("{} WHERE quantum_checksum IS NULL", select_sql);

        let rows: Vec<(Uuid, Vec<u8>, DateTime<Utc>)> = match table {
            "memories" => sqlx::query_as::<_, (Uuid, Vec<u8>, DateTime<Utc>)>(&query)
                .fetch_all(pool)
                .await?,
            "session_messages" => {
                let rows_i64: Vec<(i64, String, DateTime<Utc>)> =
                    sqlx::query_as(&query).fetch_all(pool).await?;
                rows_i64
                    .into_iter()
                    .map(|(id, content, created_at)| {
                        (Uuid::from_u128(id as u128), content.into_bytes(), created_at)
                    })
                    .collect()
            }
            "elixirs" => {
                let rows_str: Vec<(Uuid, String, DateTime<Utc>)> =
                    sqlx::query_as(&query).fetch_all(pool).await?;
                rows_str
                    .into_iter()
                    .map(|(id, content, created_at)| (id, content.into_bytes(), created_at))
                    .collect()
            }
            "task_runs" => {
                let rows_str: Vec<(Uuid, String, DateTime<Utc>)> =
                    sqlx::query_as(&query).fetch_all(pool).await?;
                rows_str
                    .into_iter()
                    .map(|(id, content, created_at)| (id, content.into_bytes(), created_at))
                    .collect()
            }
            _ => {
                return Err(MagicError::ProviderError {
                    provider: "verifier".to_string(),
                    message: format!("Unknown table: {}", table),
                })
            }
        };

        let mut backfilled_count = 0u64;

        for (row_id, content, created_at) in rows {
            match self
                .hasher
                .compute(table, row_id, &content, created_at)
                .await
            {
                Ok(checksum) => {
                    let update_query = format!(
                        "UPDATE {} SET quantum_checksum = $1 WHERE {} = $2",
                        table, pk_column
                    );

                    let result = match table {
                        "session_messages" => {
                            sqlx::query(&update_query)
                                .bind(&checksum)
                                .bind(row_id.as_u128() as i64)
                                .execute(pool)
                                .await
                        }
                        _ => {
                            sqlx::query(&update_query)
                                .bind(&checksum)
                                .bind(row_id)
                                .execute(pool)
                                .await
                        }
                    };

                    match result {
                        Ok(_) => backfilled_count += 1,
                        Err(e) => {
                            tracing::warn!(
                                "Failed to backfill checksum for {} {} in {}: {}",
                                pk_column,
                                row_id,
                                table,
                                e
                            );
                        }
                    }
                }
                Err(e) => {
                    tracing::warn!(
                        "Failed to compute checksum for {} {} in {}: {}",
                        pk_column,
                        row_id,
                        table,
                        e
                    );
                }
            }
        }

        Ok(backfilled_count)
    }
}
