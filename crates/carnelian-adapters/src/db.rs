//! Database operations for the `channel_sessions` table.
//!
//! All functions operate on the existing `channel_sessions` table defined in
//! migration `00000000000002_phase1_delta.sql`.

use serde_json::Value as JsonValue;
use sqlx::PgPool;
use uuid::Uuid;

use carnelian_common::{Error, Result};

use crate::types::ChannelSession;

/// Create a new channel session.
///
/// Returns the generated `session_id`.
pub async fn create_channel_session(
    pool: &PgPool,
    channel_type: &str,
    channel_user_id: &str,
    trust_level: &str,
    identity_id: Option<Uuid>,
    metadata: JsonValue,
) -> Result<Uuid> {
    let session_id = sqlx::query_scalar::<_, Uuid>(
        r"
        INSERT INTO channel_sessions
            (channel_type, channel_user_id, trust_level, identity_id, metadata)
        VALUES ($1, $2, $3, $4, $5)
        RETURNING session_id
        ",
    )
    .bind(channel_type)
    .bind(channel_user_id)
    .bind(trust_level)
    .bind(identity_id)
    .bind(&metadata)
    .fetch_one(pool)
    .await
    .map_err(Error::Database)?;

    Ok(session_id)
}

/// Look up a channel session by `(channel_type, channel_user_id)`.
pub async fn get_channel_session(
    pool: &PgPool,
    channel_type: &str,
    channel_user_id: &str,
) -> Result<Option<ChannelSession>> {
    let row = sqlx::query_as::<_, ChannelSession>(
        r"
        SELECT session_id, channel_type, channel_user_id, trust_level,
               identity_id, created_at, last_seen_at, metadata
        FROM channel_sessions
        WHERE channel_type = $1 AND channel_user_id = $2
        ",
    )
    .bind(channel_type)
    .bind(channel_user_id)
    .fetch_optional(pool)
    .await
    .map_err(Error::Database)?;

    Ok(row)
}

/// Look up a channel session by its `session_id`.
pub async fn get_channel_session_by_id(
    pool: &PgPool,
    session_id: Uuid,
) -> Result<Option<ChannelSession>> {
    let row = sqlx::query_as::<_, ChannelSession>(
        r"
        SELECT session_id, channel_type, channel_user_id, trust_level,
               identity_id, created_at, last_seen_at, metadata
        FROM channel_sessions
        WHERE session_id = $1
        ",
    )
    .bind(session_id)
    .fetch_optional(pool)
    .await
    .map_err(Error::Database)?;

    Ok(row)
}

/// Update a channel session's trust level and metadata.
pub async fn update_channel_session(
    pool: &PgPool,
    session_id: Uuid,
    trust_level: &str,
    metadata: JsonValue,
) -> Result<()> {
    sqlx::query(
        r"
        UPDATE channel_sessions
        SET trust_level = $2, metadata = $3, last_seen_at = NOW()
        WHERE session_id = $1
        ",
    )
    .bind(session_id)
    .bind(trust_level)
    .bind(&metadata)
    .execute(pool)
    .await
    .map_err(Error::Database)?;

    Ok(())
}

/// Delete a channel session by `session_id`.
///
/// Returns `true` if a row was deleted.
pub async fn delete_channel_session(pool: &PgPool, session_id: Uuid) -> Result<bool> {
    let result = sqlx::query(r"DELETE FROM channel_sessions WHERE session_id = $1")
        .bind(session_id)
        .execute(pool)
        .await
        .map_err(Error::Database)?;

    Ok(result.rows_affected() > 0)
}

/// List all channel sessions, optionally filtered by `channel_type`.
pub async fn list_channel_sessions(
    pool: &PgPool,
    channel_type: Option<&str>,
) -> Result<Vec<ChannelSession>> {
    let rows = if let Some(ct) = channel_type {
        sqlx::query_as::<_, ChannelSession>(
            r"
            SELECT session_id, channel_type, channel_user_id, trust_level,
                   identity_id, created_at, last_seen_at, metadata
            FROM channel_sessions
            WHERE channel_type = $1
            ORDER BY last_seen_at DESC
            ",
        )
        .bind(ct)
        .fetch_all(pool)
        .await
        .map_err(Error::Database)?
    } else {
        sqlx::query_as::<_, ChannelSession>(
            r"
            SELECT session_id, channel_type, channel_user_id, trust_level,
                   identity_id, created_at, last_seen_at, metadata
            FROM channel_sessions
            ORDER BY last_seen_at DESC
            ",
        )
        .fetch_all(pool)
        .await
        .map_err(Error::Database)?
    };

    Ok(rows)
}

/// Update the `last_seen_at` timestamp for a channel session.
pub async fn touch_channel_session(pool: &PgPool, session_id: Uuid) -> Result<()> {
    sqlx::query(r"UPDATE channel_sessions SET last_seen_at = NOW() WHERE session_id = $1")
        .bind(session_id)
        .execute(pool)
        .await
        .map_err(Error::Database)?;

    Ok(())
}

/// Upsert a channel session — create if not exists, otherwise update
/// `last_seen_at` and return the existing session.
pub async fn upsert_channel_session(
    pool: &PgPool,
    channel_type: &str,
    channel_user_id: &str,
    trust_level: &str,
    identity_id: Option<Uuid>,
    metadata: JsonValue,
) -> Result<ChannelSession> {
    let row = sqlx::query_as::<_, ChannelSession>(
        r"
        INSERT INTO channel_sessions
            (channel_type, channel_user_id, trust_level, identity_id, metadata)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (channel_type, channel_user_id)
        DO UPDATE SET last_seen_at = NOW()
        RETURNING session_id, channel_type, channel_user_id, trust_level,
                  identity_id, created_at, last_seen_at, metadata
        ",
    )
    .bind(channel_type)
    .bind(channel_user_id)
    .bind(trust_level)
    .bind(identity_id)
    .bind(&metadata)
    .fetch_one(pool)
    .await
    .map_err(Error::Database)?;

    Ok(row)
}
