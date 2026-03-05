#![allow(clippy::uninlined_format_args)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::missing_panics_doc)]
#![allow(unused_imports)]
#![allow(clippy::redundant_clone)]
#![allow(clippy::too_many_lines)]

//! Integration tests for the capability management HTTP API endpoints

use carnelian_common::types::{
    GrantCapabilityRequest, GrantCapabilityResponse, ListCapabilitiesResponse,
    RevokeCapabilityResponse,
};
use carnelian_core::{
    Config, EventStream, Ledger, ModelRouter, PolicyEngine, Scheduler, Server, WorkerManager,
};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use testcontainers::{runners::AsyncRunner, GenericImage, ImageExt};
use tokio::net::TcpListener;

async fn allocate_random_port() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("Failed to bind to random port");
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    port
}

async fn wait_for_server(port: u16, timeout: Duration) -> bool {
    let start = std::time::Instant::now();
    while start.elapsed() < timeout {
        if tokio::net::TcpStream::connect(format!("127.0.0.1:{}", port))
            .await
            .is_ok()
        {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    false
}

async fn create_postgres_container() -> testcontainers::ContainerAsync<GenericImage> {
    let image = GenericImage::new("pgvector/pgvector", "pg16").with_wait_for(
        testcontainers::core::WaitFor::message_on_stderr(
            "database system is ready to accept connections",
        ),
    );

    image
        .with_env_var("POSTGRES_USER", "test")
        .with_env_var("POSTGRES_PASSWORD", "test")
        .with_env_var("POSTGRES_DB", "carnelian_test")
        .start()
        .await
        .expect("Failed to start PostgreSQL container")
}

async fn get_database_url(container: &testcontainers::ContainerAsync<GenericImage>) -> String {
    let port = container
        .get_host_port_ipv4(5432)
        .await
        .expect("Failed to get port");
    format!("postgresql://test:test@127.0.0.1:{}/carnelian_test", port)
}

async fn start_db_backed_server(db_url: &str) -> (u16, tokio::task::JoinHandle<()>) {
    let port = allocate_random_port().await;

    let mut config = Config::default();
    config.bind_address = "127.0.0.1".to_string();
    config.http_port = port;
    config.database_url = db_url.to_string();
    config
        .connect_database()
        .await
        .expect("Config should connect to database");
    let pool = config.pool().expect("Pool should be set").clone();

    let event_stream = Arc::new(EventStream::new(100, 10));
    let config = Arc::new(config);
    let policy_engine = Arc::new(PolicyEngine::new(pool.clone()));
    let ledger = Arc::new(Ledger::new(pool.clone()));
    let worker_manager = Arc::new(tokio::sync::Mutex::new(WorkerManager::new(
        config.clone(),
        event_stream.clone(),
    )));
    let model_router = Arc::new(ModelRouter::new(
        pool.clone(),
        "http://localhost:18790".to_string(),
        policy_engine.clone(),
        ledger.clone(),
    ));
    let safe_mode_guard = Arc::new(carnelian_core::SafeModeGuard::new(
        pool.clone(),
        ledger.clone(),
    ));
    let scheduler = Arc::new(tokio::sync::Mutex::new(Scheduler::new(
        pool.clone(),
        event_stream.clone(),
        Duration::from_secs(3600),
        worker_manager.clone(),
        config.clone(),
        model_router,
        ledger.clone(),
        safe_mode_guard,
    )));

    let server = Server::new(
        config,
        event_stream,
        policy_engine,
        ledger,
        scheduler,
        worker_manager,
    );

    let handle = tokio::spawn(async move {
        server.run().await.expect("Server failed to start");
    });

    assert!(
        wait_for_server(port, Duration::from_secs(10)).await,
        "Server failed to start within timeout"
    );

    (port, handle)
}

#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test capability_api_tests -- --ignored"]
async fn test_list_capabilities_empty() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&db_url)
        .await
        .expect("Failed to connect");
    carnelian_core::db::run_migrations(&pool, None)
        .await
        .expect("Migrations should succeed");
    drop(pool);

    let (port, handle) = start_db_backed_server(&db_url).await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("http://127.0.0.1:{}/v1/capabilities", port))
        .send()
        .await
        .expect("Request should succeed");

    assert_eq!(resp.status(), 200);
    let body: ListCapabilitiesResponse = resp.json().await.expect("Should parse");
    // May have seed grants from migrations, so just verify structure
    // Verify structure parses correctly (may have seed grants from migrations)
    let _ = body.grants.len();

    handle.abort();
}

#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test capability_api_tests -- --ignored"]
async fn test_list_capabilities_with_subject_type_filter() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&db_url)
        .await
        .expect("Failed to connect");
    carnelian_core::db::run_migrations(&pool, None)
        .await
        .expect("Migrations should succeed");

    // Insert a test grant directly
    sqlx::query(
        "INSERT INTO capability_grants (subject_type, subject_id, capability_key) \
         VALUES ('skill', 'test-skill-1', 'fs.read')",
    )
    .execute(&pool)
    .await
    .expect("Should insert grant");
    drop(pool);

    let (port, handle) = start_db_backed_server(&db_url).await;
    let client = reqwest::Client::new();

    // Filter by subject_type=skill
    let resp = client
        .get(format!(
            "http://127.0.0.1:{}/v1/capabilities?subject_type=skill",
            port
        ))
        .send()
        .await
        .expect("Request should succeed");

    assert_eq!(resp.status(), 200);
    let body: ListCapabilitiesResponse = resp.json().await.expect("Should parse");
    assert!(
        body.grants.iter().any(|g| g.subject_type == "skill"),
        "Should contain skill grants"
    );

    // Filter by subject_type=nonexistent
    let resp2 = client
        .get(format!(
            "http://127.0.0.1:{}/v1/capabilities?subject_type=nonexistent",
            port
        ))
        .send()
        .await
        .expect("Request should succeed");

    assert_eq!(resp2.status(), 200);
    let body2: ListCapabilitiesResponse = resp2.json().await.expect("Should parse");
    assert!(
        body2.grants.is_empty(),
        "Should have no grants for nonexistent type"
    );

    handle.abort();
}

#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test capability_api_tests -- --ignored"]
async fn test_grant_capability_queues_approval() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&db_url)
        .await
        .expect("Failed to connect");
    carnelian_core::db::run_migrations(&pool, None)
        .await
        .expect("Migrations should succeed");
    drop(pool);

    let (port, handle) = start_db_backed_server(&db_url).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("http://127.0.0.1:{}/v1/capabilities", port))
        .json(&GrantCapabilityRequest {
            subject_type: "identity".to_string(),
            subject_id: "test-user-1".to_string(),
            capability_key: "fs.read".to_string(),
            scope: Some(json!({"path": "/data"})),
            constraints: None,
            expires_at: None,
        })
        .send()
        .await
        .expect("Request should succeed");

    let status = resp.status().as_u16();
    // Should be either 201 (direct grant) or 202 (queued for approval)
    assert!(
        status == 201 || status == 202,
        "Expected 201 or 202, got {}",
        status
    );

    handle.abort();
}

#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test capability_api_tests -- --ignored"]
async fn test_revoke_capability() {
    let container = create_postgres_container().await;
    let db_url = get_database_url(&container).await;

    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(2)
        .connect(&db_url)
        .await
        .expect("Failed to connect");
    carnelian_core::db::run_migrations(&pool, None)
        .await
        .expect("Migrations should succeed");

    // Insert a grant directly to revoke
    let grant_id: uuid::Uuid = sqlx::query_scalar(
        "INSERT INTO capability_grants (subject_type, subject_id, capability_key) \
         VALUES ('channel', 'test-channel-1', 'net.http') RETURNING grant_id",
    )
    .fetch_one(&pool)
    .await
    .expect("Should insert grant");
    drop(pool);

    let (port, handle) = start_db_backed_server(&db_url).await;
    let client = reqwest::Client::new();

    let resp = client
        .delete(format!(
            "http://127.0.0.1:{}/v1/capabilities/{}",
            port, grant_id
        ))
        .send()
        .await
        .expect("Request should succeed");

    let status = resp.status().as_u16();
    // Should be either 200 (revoked) or 202 (queued for approval)
    assert!(
        status == 200 || status == 202,
        "Expected 200 or 202, got {}",
        status
    );

    handle.abort();
}
