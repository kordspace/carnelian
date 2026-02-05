//! Database module for Carnelian OS
//!
//! Provides database connection management, health checks, migration running,
//! and reconnection logic with exponential backoff.

use carnelian_common::{Error, Result};
use sqlx::PgPool;
use std::time::Duration;
use tokio::time::sleep;

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
            tracing::warn!(timeout_secs = HEALTH_CHECK_TIMEOUT_SECS, "Database health check timed out");
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
                        tracing::info!(
                            attempt = attempt,
                            "Database reconnection successful"
                        );
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
/// # Arguments
/// * `pool` - Reference to the database connection pool
///
/// # Returns
/// * `Ok(())` if migrations complete successfully
/// * `Err` if migration fails
///
/// # Example
/// ```ignore
/// run_migrations(&pool).await?;
/// ```
pub async fn run_migrations(pool: &PgPool) -> Result<()> {
    tracing::info!(migrations_path = "db/migrations", "Running database migrations");

    sqlx::migrate!("../../db/migrations").run(pool).await?;

    tracing::info!("Database migrations completed successfully");
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
        let pool = PgPool::connect("postgresql://carnelian:carnelian@localhost:5432/carnelian")
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
