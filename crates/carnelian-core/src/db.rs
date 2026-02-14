//! Database module for Carnelian OS
//!
//! Provides database connection management, health checks, migration running,
//! and reconnection logic with exponential backoff.

use carnelian_common::{Error, Result};
use serde_json::json;
use sqlx::PgPool;
use std::time::Duration;
use tokio::time::sleep;
use uuid::Uuid;

/// Re-export PgPool for convenience
pub use sqlx::PgPool as Pool;

/// Health check timeout in seconds
const HEALTH_CHECK_TIMEOUT_SECS: u64 = 5;

/// Maximum reconnection attempts
const MAX_RECONNECT_ATTEMPTS: u32 = 5;

/// Base delay for exponential backoff (in seconds)
const BASE_RECONNECT_DELAY_SECS: u64 = 1;

/// Check database health by executing a simple query.
///
/// # Arguments
/// * `pool` - Reference to the database connection pool
///
/// # Returns
/// * `Ok(true)` if the database is healthy
/// * `Ok(false)` if the health check fails but doesn't error
/// * `Err` if there's a connection error
///
/// # Example
/// ```ignore
/// let healthy = check_database_health(&pool).await?;
/// if !healthy {
///     tracing::warn!("Database health check failed");
/// }
/// ```
pub async fn check_database_health(pool: &PgPool) -> Result<bool> {
    let result = tokio::time::timeout(
        Duration::from_secs(HEALTH_CHECK_TIMEOUT_SECS),
        sqlx::query_scalar::<_, i32>("SELECT 1").fetch_one(pool),
    )
    .await;

    match result {
        Ok(Ok(1)) => {
            tracing::debug!("Database health check passed");
            Ok(true)
        }
        Ok(Ok(_)) => Ok(false), // Unexpected result
        Ok(Err(e)) => {
            tracing::warn!(error = %e, "Database health check query failed");
            Ok(false)
        }
        Err(_) => {
            tracing::warn!(
                timeout_secs = HEALTH_CHECK_TIMEOUT_SECS,
                "Database health check timed out"
            );
            Ok(false)
        }
    }
}

/// Ensure database connection is healthy, attempting reconnection if needed.
///
/// Uses exponential backoff with delays: 1s, 2s, 4s, 8s, 16s
///
/// # Arguments
/// * `config` - Mutable reference to the application config
///
/// # Returns
/// * `Ok(())` if connection is healthy or successfully reconnected
/// * `Err` if max retries exceeded
///
/// # Example
/// ```ignore
/// ensure_database_connection(&mut config).await?;
/// ```
pub async fn ensure_database_connection(config: &mut crate::config::Config) -> Result<()> {
    // Check if already connected and healthy
    if config.is_connected() {
        if let Ok(pool) = config.pool() {
            if check_database_health(pool).await? {
                return Ok(());
            }
        }
    }

    // Attempt reconnection with exponential backoff
    for attempt in 1..=MAX_RECONNECT_ATTEMPTS {
        let delay = Duration::from_secs(BASE_RECONNECT_DELAY_SECS * 2u64.pow(attempt - 1));

        tracing::warn!(
            attempt = attempt,
            max_attempts = MAX_RECONNECT_ATTEMPTS,
            delay_secs = delay.as_secs(),
            "Attempting database reconnection"
        );

        match config.connect_database().await {
            Ok(()) => {
                // Verify the new connection is healthy
                if let Ok(pool) = config.pool() {
                    if check_database_health(pool).await? {
                        tracing::info!(attempt = attempt, "Database reconnection successful");
                        return Ok(());
                    }
                }
            }
            Err(e) => {
                tracing::warn!(
                    attempt = attempt,
                    error = %e,
                    "Reconnection attempt failed"
                );
            }
        }

        if attempt < MAX_RECONNECT_ATTEMPTS {
            tracing::debug!(
                delay_secs = delay.as_secs(),
                "Waiting before next reconnection attempt"
            );
            sleep(delay).await;
        }
    }

    Err(Error::Connection(format!(
        "Failed to establish database connection after {} attempts",
        MAX_RECONNECT_ATTEMPTS
    )))
}

/// Run database migrations.
///
/// Applies all pending migrations from the `db/migrations` directory.
///
/// When an `approval_queue` is provided, pending migrations are **not** executed
/// directly. Instead the latest pending version is queued for approval and
/// `Error::ApprovalRequired` is returned so the caller can track the request.
/// Pass `None` to run migrations immediately (original behaviour).
///
/// # Arguments
/// * `pool` - Reference to the database connection pool
/// * `approval_queue` - Optional approval queue; when `Some`, migrations require approval
///
/// # Returns
/// * `Ok(())` if migrations complete successfully (or no pending migrations)
/// * `Err(Error::ApprovalRequired(id))` if migrations were queued for approval
/// * `Err` if migration fails
pub async fn run_migrations(
    pool: &PgPool,
    approval_queue: Option<&crate::approvals::ApprovalQueue>,
) -> Result<()> {
    let migrator = sqlx::migrate!("../../db/migrations");

    if let Some(queue) = approval_queue {
        // Determine pending migrations
        let applied_versions: std::collections::HashSet<i64> =
            sqlx::query_scalar("SELECT version FROM _sqlx_migrations")
                .fetch_all(pool)
                .await
                .unwrap_or_default()
                .into_iter()
                .collect();

        let latest_pending = migrator
            .iter()
            .filter(|m| !m.migration_type.is_down_migration())
            .filter(|m| !applied_versions.contains(&m.version))
            .map(|m| m.version)
            .max();

        if let Some(version) = latest_pending {
            let approval_id = queue_migration(queue, &version.to_string(), None).await?;
            return Err(carnelian_common::Error::ApprovalRequired(approval_id));
        }

        // No pending migrations — nothing to approve
        tracing::info!("No pending migrations, database is up to date");
        return Ok(());
    }

    tracing::info!(
        migrations_path = "db/migrations",
        "Running database migrations"
    );

    migrator.run(pool).await?;

    tracing::info!("Database migrations completed successfully");
    Ok(())
}

/// Queue a database migration for approval before execution.
///
/// Creates an approval request containing the migration version string.
/// Returns the approval ID for tracking.
pub async fn queue_migration(
    approval_queue: &crate::approvals::ApprovalQueue,
    migration_version: &str,
    correlation_id: Option<Uuid>,
) -> Result<Uuid> {
    let payload = json!({
        "migration_version": migration_version,
    });
    approval_queue
        .queue_action("db.migration", payload, None, correlation_id)
        .await
}

/// Execute a previously approved database migration.
///
/// Fetches the approval request, verifies it is approved, parses the
/// approved `migration_version` as an `i64`, validates that no pending
/// migration exceeds the approved version, and runs migrations only up
/// to that version using `Migrator::run_to`. Logs exactly the approved
/// version to the ledger on success.
pub async fn execute_approved_migration(
    pool: &PgPool,
    approval_id: Uuid,
    approval_queue: &crate::approvals::ApprovalQueue,
    ledger: &crate::ledger::Ledger,
    owner_signing_key: Option<&ed25519_dalek::SigningKey>,
) -> Result<()> {
    let request = approval_queue
        .get(approval_id)
        .await?
        .ok_or_else(|| Error::Security(format!("Approval request not found: {}", approval_id)))?;

    if request.status != "approved" {
        return Err(Error::Security(format!(
            "Approval request {} is not approved (status: {})",
            approval_id, request.status
        )));
    }

    let payload = &request.payload;
    let migration_version_str = payload["migration_version"].as_str().ok_or_else(|| {
        Error::Security("Missing migration_version in approval payload".to_string())
    })?;

    let approved_version: i64 = migration_version_str.parse().map_err(|e| {
        Error::Security(format!(
            "Invalid migration_version '{}': {}",
            migration_version_str, e
        ))
    })?;

    let migrator = sqlx::migrate!("../../db/migrations");

    // Collect applied versions from the database
    let applied_versions: std::collections::HashSet<i64> =
        sqlx::query_scalar("SELECT version FROM _sqlx_migrations")
            .fetch_all(pool)
            .await
            .unwrap_or_default()
            .into_iter()
            .collect();

    // Determine pending migrations
    let pending: Vec<_> = migrator
        .iter()
        .filter(|m| !m.migration_type.is_down_migration())
        .filter(|m| !applied_versions.contains(&m.version))
        .collect();

    // Verify no pending migration exceeds the approved version
    for m in &pending {
        if m.version > approved_version {
            return Err(Error::Security(format!(
                "Pending migration V{} ('{}') exceeds approved version V{}; \
                 queue a separate approval for that version",
                m.version, m.description, approved_version
            )));
        }
    }

    // Build a filtered migrator containing only migrations up to the approved version.
    // The Migrator fields are doc(hidden) but pub for macro use; this is the only way
    // to construct a version-bounded migrator in sqlx 0.8.x which lacks run_to().
    let filtered_migrations: Vec<_> = migrator
        .iter()
        .filter(|m| m.version <= approved_version)
        .cloned()
        .collect();

    let filtered_migrator = sqlx::migrate::Migrator {
        migrations: std::borrow::Cow::Owned(filtered_migrations),
        ignore_missing: migrator.ignore_missing,
        locking: migrator.locking,
        no_tx: migrator.no_tx,
    };

    tracing::info!(
        approved_version = approved_version,
        pending_count = pending.len(),
        "Running approved migrations up to version"
    );

    filtered_migrator
        .run(pool)
        .await
        .map_err(|e| Error::Migration(e))?;

    tracing::info!(
        migration_version = %migration_version_str,
        approval_id = %approval_id,
        "Approved migration executed"
    );

    ledger
        .log_migration(migration_version_str, owner_signing_key)
        .await?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_health_check_with_valid_pool() {
        // This test requires a running PostgreSQL instance
        // Run with: cargo test --package carnelian-core -- --ignored
        let db_url = std::env::var("DATABASE_URL")
            .unwrap_or_else(|_| "postgresql://carnelian:carnelian@localhost:5432/carnelian".into());
        let pool = PgPool::connect(&db_url)
            .await
            .expect("Failed to connect to database");

        let result = check_database_health(&pool).await;
        assert!(result.is_ok());
        assert!(result.unwrap());
    }

    #[tokio::test]
    async fn test_health_check_constants() {
        assert_eq!(HEALTH_CHECK_TIMEOUT_SECS, 5);
        assert_eq!(MAX_RECONNECT_ATTEMPTS, 5);
        assert_eq!(BASE_RECONNECT_DELAY_SECS, 1);
    }
}
