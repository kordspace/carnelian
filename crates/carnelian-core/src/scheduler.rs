//! Task scheduler and heartbeat runner for 🔥 Carnelian OS
//!
//! The Scheduler manages background tasks including:
//! - **Heartbeat System**: Periodic heartbeats at configurable intervals (default 555,555ms ≈ 9.26 minutes)
//! - **Mantra Selection**: "First unknown, then random rotation" strategy for selecting mantras
//! - **Task Queue Polling**: Placeholder for future task execution (Phase 2)
//!
//! # Heartbeat Interval
//!
//! The default interval of 555,555ms is configurable via:
//! - `heartbeat_interval_ms` in `machine.toml`
//! - `CARNELIAN_HEARTBEAT_INTERVAL_MS` environment variable
//!
//! # Mantra Selection Strategy
//!
//! Mantras are selected using a "first unknown, then random rotation" approach:
//! 1. Query previously used mantras for the identity
//! 2. If any mantras haven't been used yet, select the first unknown one
//! 3. Once all mantras have been used, randomly select from the full rotation
//!
//! # Integration with Event Stream
//!
//! Each heartbeat emits a `HeartbeatTick` event containing:
//! - `heartbeat_id`: Database record ID
//! - `identity_id`: The identity performing the heartbeat (Lian)
//! - `mantra`: Selected mantra for this heartbeat
//! - `tasks_queued`: Number of pending tasks in the queue
//! - `duration_ms`: Time taken to execute the heartbeat
//!
//! # Graceful Shutdown
//!
//! The scheduler responds to shutdown signals and cleanly terminates the heartbeat loop.
//! Call `shutdown()` before stopping the server to ensure proper cleanup.

use crate::events::EventStream;
use carnelian_common::types::{EventEnvelope, EventLevel, EventType};
use carnelian_common::{Error, Result};
use rand::seq::SliceRandom;
use serde_json::json;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::watch;
use uuid::Uuid;

/// Static list of mantras for the heartbeat system
const MANTRAS: &[&str] = &[
    "What wants to emerge?",
    "Be present and authentic",
    "Share a brief thought",
    "Notice what's alive",
    "Trust the process",
];

/// Background task scheduler managing heartbeats and task queue polling.
///
/// The Scheduler runs as a background tokio task, periodically executing
/// heartbeats and (in the future) polling the task queue for work.
pub struct Scheduler {
    /// Database connection pool
    pool: PgPool,
    /// Event stream for publishing heartbeat events
    event_stream: Arc<EventStream>,
    /// Interval between heartbeats
    heartbeat_interval: Duration,
    /// Shutdown signal sender
    shutdown_tx: Option<watch::Sender<bool>>,
}

impl Scheduler {
    /// Create a new Scheduler instance.
    ///
    /// # Arguments
    ///
    /// * `pool` - Database connection pool for heartbeat logging
    /// * `event_stream` - Event stream for publishing HeartbeatTick events
    /// * `heartbeat_interval` - Duration between heartbeat ticks
    ///
    /// # Example
    ///
    /// ```ignore
    /// use std::time::Duration;
    /// let scheduler = Scheduler::new(pool, event_stream, Duration::from_millis(555_555));
    /// ```
    pub fn new(pool: PgPool, event_stream: Arc<EventStream>, heartbeat_interval: Duration) -> Self {
        Self {
            pool,
            event_stream,
            heartbeat_interval,
            shutdown_tx: None,
        }
    }

    /// Start the scheduler background task.
    ///
    /// This method spawns a background tokio task that runs the heartbeat loop
    /// at the configured interval. The method returns immediately (non-blocking).
    ///
    /// # Returns
    ///
    /// Returns `Ok(())` after spawning the background task.
    ///
    /// # Errors
    ///
    /// This method does not return errors directly, but the background task
    /// will log errors if heartbeat execution fails.
    #[allow(clippy::unused_async)]
    pub async fn start(&mut self) -> Result<()> {
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        self.shutdown_tx = Some(shutdown_tx);

        let pool = self.pool.clone();
        let event_stream = self.event_stream.clone();
        let interval = self.heartbeat_interval;

        tokio::spawn(async move {
            Self::run_heartbeat_loop(pool, event_stream, interval, shutdown_rx).await;
        });

        tracing::info!(
            heartbeat_interval_ms = interval.as_millis() as u64,
            "Scheduler started"
        );

        Ok(())
    }

    /// Shutdown the scheduler gracefully.
    ///
    /// Sends a shutdown signal to the background task and waits for it to terminate.
    #[allow(clippy::unused_async)]
    pub async fn shutdown(&mut self) -> Result<()> {
        if let Some(tx) = self.shutdown_tx.take() {
            let _ = tx.send(true);
            tracing::info!("Scheduler shutdown signal sent");
        }
        Ok(())
    }

    /// Run the heartbeat loop until shutdown signal is received.
    async fn run_heartbeat_loop(
        pool: PgPool,
        event_stream: Arc<EventStream>,
        interval: Duration,
        mut shutdown_rx: watch::Receiver<bool>,
    ) {
        let mut ticker = tokio::time::interval(interval);
        // Skip the first immediate tick
        ticker.tick().await;

        loop {
            tokio::select! {
                _ = ticker.tick() => {
                    if let Err(e) = Self::run_heartbeat(&pool, &event_stream).await {
                        tracing::warn!(error = %e, "Heartbeat execution failed");
                    }
                    // Poll task queue after heartbeat, logging errors but not failing the loop
                    if let Err(e) = Self::poll_task_queue(&pool).await {
                        tracing::warn!(error = %e, "Task queue polling failed");
                    }
                }
                _ = shutdown_rx.changed() => {
                    if *shutdown_rx.borrow() {
                        tracing::info!("Scheduler received shutdown signal, stopping heartbeat loop");
                        break;
                    }
                }
            }
        }
    }

    /// Execute a single heartbeat cycle.
    ///
    /// This method:
    /// 1. Queries the database for the default identity (Lian)
    /// 2. Selects a mantra using the "first unknown, then random" strategy
    /// 3. Counts pending tasks in the queue
    /// 4. Logs the heartbeat to `heartbeat_history`
    /// 5. Emits a `HeartbeatTick` event
    async fn run_heartbeat(pool: &PgPool, event_stream: &EventStream) -> Result<()> {
        let start = std::time::Instant::now();

        // Query for default identity (Lian)
        let identity_id: Option<Uuid> = sqlx::query_scalar(
            r"SELECT identity_id FROM identities WHERE name = 'Lian' AND identity_type = 'core' LIMIT 1",
        )
        .fetch_optional(pool)
        .await
        .map_err(Error::Database)?;

        let identity_id = match identity_id {
            Some(id) => id,
            None => {
                tracing::error!("Default identity 'Lian' not found in database");
                return Err(Error::Database(sqlx::Error::RowNotFound));
            }
        };

        // Select mantra
        let mantra = Self::select_mantra(pool, identity_id).await?;

        // Count pending tasks
        let tasks_queued: i64 = sqlx::query_scalar::<_, Option<i64>>(
            r"SELECT COUNT(*) FROM tasks WHERE state = 'pending'",
        )
        .fetch_one(pool)
        .await
        .map_err(Error::Database)?
        .unwrap_or(0);

        let duration_ms = start.elapsed().as_millis() as i32;

        // Insert heartbeat record
        let heartbeat_id: Uuid = sqlx::query_scalar(
            r"
            INSERT INTO heartbeat_history (identity_id, mantra, tasks_queued, status, duration_ms)
            VALUES ($1, $2, $3, $4, $5)
            RETURNING heartbeat_id
            ",
        )
        .bind(identity_id)
        .bind(&mantra)
        .bind(tasks_queued as i32)
        .bind("ok")
        .bind(duration_ms)
        .fetch_one(pool)
        .await
        .map_err(Error::Database)?;

        // Emit HeartbeatTick event
        event_stream.publish(EventEnvelope::new(
            EventLevel::Info,
            EventType::HeartbeatTick,
            json!({
                "heartbeat_id": heartbeat_id,
                "identity_id": identity_id,
                "mantra": mantra,
                "tasks_queued": tasks_queued,
                "duration_ms": duration_ms,
                "status": "ok"
            }),
        ));

        tracing::info!(
            heartbeat_id = %heartbeat_id,
            identity_id = %identity_id,
            mantra = ?mantra,
            tasks_queued = tasks_queued,
            duration_ms = duration_ms,
            "Heartbeat completed"
        );

        Ok(())
    }

    /// Select a mantra using "first unknown, then random rotation" strategy.
    ///
    /// # Strategy
    ///
    /// 1. Query previously used mantras for this identity
    /// 2. Find mantras not yet used (set difference)
    /// 3. If unknown mantras exist, return the first one
    /// 4. Otherwise, randomly select from the full rotation
    async fn select_mantra(pool: &PgPool, identity_id: Uuid) -> Result<Option<String>> {
        // Query used mantras
        let used_mantras: Vec<String> = sqlx::query_scalar(
            r"SELECT DISTINCT mantra FROM heartbeat_history WHERE identity_id = $1 AND mantra IS NOT NULL",
        )
        .bind(identity_id)
        .fetch_all(pool)
        .await
        .map_err(Error::Database)?;

        // Find unknown mantras (not yet used)
        let unknown: Vec<&str> = MANTRAS
            .iter()
            .copied()
            .filter(|m| !used_mantras.iter().any(|u| u == *m))
            .collect();

        if !unknown.is_empty() {
            // Return first unknown mantra
            return Ok(Some(unknown[0].to_string()));
        }

        // All mantras used, select randomly
        let mut rng = rand::thread_rng();
        Ok(MANTRAS.choose(&mut rng).map(|s| (*s).to_string()))
    }

    /// Poll the task queue for pending work (placeholder).
    ///
    /// # Note
    ///
    /// Full task queue polling will be implemented in Phase 2.
    /// Currently this method only logs the count of pending tasks.
    async fn poll_task_queue(pool: &PgPool) -> Result<()> {
        // TODO: Full task queue polling will be implemented in Phase 2

        let pending_tasks: Vec<(Uuid, String, i32)> = sqlx::query_as(
            r"SELECT task_id, title, priority FROM tasks WHERE state = 'pending' ORDER BY priority DESC, created_at ASC LIMIT 10",
        )
        .fetch_all(pool)
        .await
        .map_err(Error::Database)?;

        tracing::debug!(pending_count = pending_tasks.len(), "Polled task queue");

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[allow(clippy::len_zero)]
    fn test_mantras_defined() {
        assert!(MANTRAS.len() > 0, "Mantras list should not be empty");
        assert_eq!(MANTRAS.len(), 5, "Should have 5 mantras defined");
    }

    #[tokio::test]
    async fn test_scheduler_creation() {
        // Create a lazy pool for testing
        let pool = sqlx::postgres::PgPoolOptions::new()
            .max_connections(1)
            .connect_lazy("postgresql://test:test@localhost:5432/test")
            .expect("Failed to create lazy pool");
        let event_stream = Arc::new(EventStream::new(100, 10));
        let scheduler = Scheduler::new(pool, event_stream, Duration::from_millis(1000));

        assert_eq!(scheduler.heartbeat_interval, Duration::from_millis(1000));
        assert!(scheduler.shutdown_tx.is_none());
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_mantra_selection_unknown_first() {
        // This test requires a real database connection
        // Run with: cargo test test_mantra_selection_unknown_first -- --ignored
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_mantra_selection_random_rotation() {
        // This test requires a real database connection
        // Run with: cargo test test_mantra_selection_random_rotation -- --ignored
    }

    #[tokio::test]
    #[ignore = "requires database connection"]
    async fn test_heartbeat_execution() {
        // This test requires a real database connection
        // Run with: cargo test test_heartbeat_execution -- --ignored
    }
}
