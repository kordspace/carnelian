//! Configuration management

use carnelian_common::{Error, Result};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::sync::Arc;
use std::time::Duration;

/// Application configuration with optional database pool
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    pub database_url: String,
    pub http_port: u16,
    pub ws_port: u16,
    pub ollama_url: String,
    pub machine_profile: MachineProfile,
    /// Maximum database connections (default: 10)
    #[serde(default = "default_max_connections")]
    pub db_max_connections: u32,
    /// Database connection timeout in seconds (default: 30)
    #[serde(default = "default_connection_timeout")]
    pub db_connection_timeout_secs: u64,
    /// Database idle timeout in seconds (default: 600 = 10 minutes)
    #[serde(default = "default_idle_timeout")]
    pub db_idle_timeout_secs: u64,
    /// Database pool (not serialized, initialized separately)
    #[serde(skip)]
    db_pool: Option<Arc<PgPool>>,
}

fn default_max_connections() -> u32 {
    10
}

fn default_connection_timeout() -> u64 {
    30
}

fn default_idle_timeout() -> u64 {
    600
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum MachineProfile {
    Thummim, // 2080 Super, 32GB RAM (constrained)
    Urim,    // 2080 Ti, 64GB RAM (high-end)
}

impl Default for Config {
    fn default() -> Self {
        Self {
            database_url: "postgresql://carnelian:carnelian@localhost:5432/carnelian".to_string(),
            http_port: 3000,
            ws_port: 3001,
            ollama_url: "http://localhost:11434".to_string(),
            machine_profile: MachineProfile::Thummim,
            db_max_connections: default_max_connections(),
            db_connection_timeout_secs: default_connection_timeout(),
            db_idle_timeout_secs: default_idle_timeout(),
            db_pool: None,
        }
    }
}

impl Config {
    /// Connect to the database and initialize the connection pool.
    ///
    /// # Errors
    /// Returns an error if the connection cannot be established.
    pub async fn connect_database(&mut self) -> Result<()> {
        tracing::info!("Connecting to database...");

        let pool = PgPoolOptions::new()
            .max_connections(self.db_max_connections)
            .acquire_timeout(Duration::from_secs(self.db_connection_timeout_secs))
            .idle_timeout(Duration::from_secs(self.db_idle_timeout_secs))
            .connect(&self.database_url)
            .await?;

        tracing::info!("Database connection pool established");
        self.db_pool = Some(Arc::new(pool));
        Ok(())
    }

    /// Get a reference to the database pool.
    ///
    /// # Errors
    /// Returns an error if the database is not connected.
    pub fn pool(&self) -> Result<&PgPool> {
        self.db_pool
            .as_ref()
            .map(|p| p.as_ref())
            .ok_or_else(|| Error::Connection("Database not connected".to_string()))
    }

    /// Get an Arc reference to the database pool for sharing across tasks.
    ///
    /// # Errors
    /// Returns an error if the database is not connected.
    pub fn pool_arc(&self) -> Result<Arc<PgPool>> {
        self.db_pool
            .clone()
            .ok_or_else(|| Error::Connection("Database not connected".to_string()))
    }

    /// Check if the database pool is initialized.
    #[must_use]
    pub const fn is_connected(&self) -> bool {
        self.db_pool.is_some()
    }
}
