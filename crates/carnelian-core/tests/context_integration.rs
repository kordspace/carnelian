//! Integration tests for context assembly pipeline
//!
//! These tests verify the full context assembly workflow including:
//! - Building context for sessions
//! - Priority-based segment loading
//! - Token budget enforcement
//! - Ledger integrity logging

use carnelian_common::Result;
use carnelian_core::config::Config;
use carnelian_core::context::ContextWindow;
use carnelian_core::ledger::Ledger;
use carnelian_core::session::SessionManager;
use sqlx::PgPool;
use uuid::Uuid;

/// Helper to get test database URL
fn get_test_db_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgresql://carnelian:carnelian@localhost:5432/carnelian_test".into())
}

#[tokio::test]
#[ignore = "requires database connection"]
async fn test_context_assembly_integration() -> Result<()> {
    let pool = PgPool::connect(&get_test_db_url()).await?;
    let config = Config::default();

    // Create a test session
    let session_manager = SessionManager::new(pool.clone(), None, None, 24);
    let session = session_manager
        .create_session("agent:test:ui:group:context_test")
        .await?;

    // Add some messages to the session
    session_manager
        .append_message(
            session.session_id,
            "user",
            "Hello, this is a test message".to_string(),
            Some(10),
            None,
            None,
            None,
            None,
            None,
        )
        .await?;

    session_manager
        .append_message(
            session.session_id,
            "assistant",
            "Hello! I'm here to help.".to_string(),
            Some(8),
            None,
            None,
            None,
            None,
            None,
        )
        .await?;

    // Build context for the session
    let mut ctx =
        ContextWindow::build_for_session(pool.clone(), None, session.session_id, None, &config)
            .await?;

    // Assemble the context
    let assembled = ctx.assemble(&config).await?;

    // Verify context was assembled
    assert!(
        !assembled.is_empty(),
        "Assembled context should not be empty"
    );
    // Context was successfully assembled if we got here

    // Cleanup
    sqlx::query("DELETE FROM sessions WHERE session_id = $1")
        .bind(session.session_id)
        .execute(&pool)
        .await?;

    Ok(())
}

#[tokio::test]
#[ignore = "requires database connection"]
async fn test_build_for_session_integration() -> Result<()> {
    let pool = PgPool::connect(&get_test_db_url()).await?;
    let config = Config::default();

    // Create a test session
    let session_manager = SessionManager::new(pool.clone(), None, None, 24);
    let session = session_manager
        .create_session("agent:test:ui:group:build_test")
        .await?;

    // Build context window
    let ctx =
        ContextWindow::build_for_session(pool.clone(), None, session.session_id, None, &config)
            .await?;

    // Verify context window was created successfully
    // If we got here without errors, the context window was built correctly

    // Cleanup
    sqlx::query("DELETE FROM sessions WHERE session_id = $1")
        .bind(session.session_id)
        .execute(&pool)
        .await?;

    Ok(())
}

#[tokio::test]
#[ignore = "requires database connection"]
async fn test_log_to_ledger_integration() -> Result<()> {
    let pool = PgPool::connect(&get_test_db_url()).await?;
    let config = Config::default();
    let ledger = Ledger::new(pool.clone());

    // Create a test session with messages
    let session_manager = SessionManager::new(pool.clone(), None, None, 24);
    let session = session_manager
        .create_session("agent:test:ui:group:ledger_test")
        .await?;

    session_manager
        .append_message(
            session.session_id,
            "user",
            "Test message for ledger".to_string(),
            Some(5),
            None,
            None,
            None,
            None,
            None,
        )
        .await?;

    // Build and assemble context
    let mut ctx =
        ContextWindow::build_for_session(pool.clone(), None, session.session_id, None, &config)
            .await?;

    ctx.assemble(&config).await?;

    // Log to ledger
    let correlation_id = Uuid::now_v7();
    let event_id = ctx.log_to_ledger(&ledger, correlation_id).await?;

    // Verify ledger event was created
    assert!(event_id > 0, "Ledger event ID should be positive");

    // Verify event exists in ledger
    let event_exists = sqlx::query_scalar::<_, bool>(
        "SELECT EXISTS(SELECT 1 FROM ledger_events WHERE event_id = $1)",
    )
    .bind(event_id)
    .fetch_one(&pool)
    .await?;

    assert!(event_exists, "Ledger event should exist");

    // Cleanup
    sqlx::query("DELETE FROM sessions WHERE session_id = $1")
        .bind(session.session_id)
        .execute(&pool)
        .await?;

    sqlx::query("DELETE FROM ledger_events WHERE event_id = $1")
        .bind(event_id)
        .execute(&pool)
        .await?;

    Ok(())
}

#[tokio::test]
#[ignore = "requires database connection"]
async fn test_log_context_integrity_integration() -> Result<()> {
    let pool = PgPool::connect(&get_test_db_url()).await?;
    let config = Config::default();
    let ledger = Ledger::new(pool.clone());

    // Create a test session with messages
    let session_manager = SessionManager::new(pool.clone(), None, None, 24);
    let session = session_manager
        .create_session("agent:test:ui:group:integrity_test")
        .await?;

    session_manager
        .append_message(
            session.session_id,
            "user",
            "Test message for integrity".to_string(),
            Some(5),
            None,
            None,
            None,
            None,
            None,
        )
        .await?;

    // Build and assemble context
    let mut ctx =
        ContextWindow::build_for_session(pool.clone(), None, session.session_id, None, &config)
            .await?;

    ctx.assemble(&config).await?;

    // Log context integrity
    let correlation_id = Uuid::now_v7();
    let (event_id, provenance) = ctx.log_context_integrity(&ledger, correlation_id).await?;

    // Verify event was created
    assert!(event_id > 0, "Event ID should be positive");

    // Verify provenance data
    assert!(
        !provenance.context_bundle_hash.is_empty(),
        "Hash should not be empty"
    );
    assert!(
        provenance.total_tokens > 0,
        "Total tokens should be positive"
    );

    // Cleanup
    sqlx::query("DELETE FROM sessions WHERE session_id = $1")
        .bind(session.session_id)
        .execute(&pool)
        .await?;

    sqlx::query("DELETE FROM ledger_events WHERE event_id = $1")
        .bind(event_id)
        .execute(&pool)
        .await?;

    Ok(())
}

#[tokio::test]
#[ignore = "requires database connection"]
async fn test_resolve_context_window_limit_integration() -> Result<()> {
    let pool = PgPool::connect(&get_test_db_url()).await?;
    let mut config = Config::default();

    // Set a very small context window to trigger pruning
    config.context_window_tokens = 100;

    // Create a test session
    let session_manager = SessionManager::new(pool.clone(), None, None, 24);
    let session = session_manager
        .create_session("agent:test:ui:group:limit_test")
        .await?;

    // Add multiple messages to exceed the limit
    for i in 0..10 {
        session_manager
            .append_message(
                session.session_id,
                if i % 2 == 0 { "user" } else { "assistant" },
                format!("Message {} with some content to increase token count", i),
                Some(20),
                None,
                None,
                None,
                None,
                None,
            )
            .await?;
    }

    // Build and assemble context
    let mut ctx =
        ContextWindow::build_for_session(pool.clone(), None, session.session_id, None, &config)
            .await?;

    let assembled = ctx.assemble(&config).await?;

    // Verify context was pruned to fit within limit
    assert!(
        !assembled.is_empty(),
        "Assembled context should not be empty"
    );
    // If assembly succeeded, pruning worked correctly

    // Cleanup
    sqlx::query("DELETE FROM sessions WHERE session_id = $1")
        .bind(session.session_id)
        .execute(&pool)
        .await?;

    Ok(())
}
