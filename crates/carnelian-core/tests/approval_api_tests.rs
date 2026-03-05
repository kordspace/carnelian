#![allow(clippy::uninlined_format_args)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::missing_panics_doc)]
#![allow(unused_imports)]
#![allow(clippy::redundant_clone)]
#![allow(clippy::too_many_lines)]

//! Integration tests for the approval queue HTTP API endpoints

use carnelian_common::types::{
    ApprovalActionRequest, ApprovalActionResponse, BatchApprovalRequest, BatchApprovalResponse,
    ListApprovalsResponse,
};
use carnelian_core::{
    Config, EventStream, Ledger, ModelRouter, PolicyEngine, Scheduler, Server, WorkerManager,
};
use serde_json::json;
use std::sync::Arc;
use std::time::Duration;
use testcontainers::{runners::AsyncRunner, GenericImage, ImageExt};
use tokio::net::TcpListener;
use uuid::Uuid;

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

async fn start_db_backed_server(db_url: &str) -> (u16, tokio::task::JoinHandle<()>, Arc<Config>) {
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

    // Generate owner keypair for approval signature verification
    config
        .generate_and_store_owner_keypair(None)
        .await
        .expect("Should generate owner keypair");

    let event_stream = Arc::new(EventStream::new(100, 10));
    let config_arc = Arc::new(config);
    let policy_engine = Arc::new(PolicyEngine::new(pool.clone()));
    let ledger = Arc::new(Ledger::new(pool.clone()));
    let worker_manager = Arc::new(tokio::sync::Mutex::new(WorkerManager::new(
        config_arc.clone(),
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
        config_arc.clone(),
        model_router,
        ledger.clone(),
        safe_mode_guard,
    )));

    let server = Server::new(
        config_arc.clone(),
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

    (port, handle, config_arc)
}

#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test approval_api_tests -- --ignored"]
async fn test_list_approvals_empty() {
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

    let (port, handle, _config) = start_db_backed_server(&db_url).await;
    let client = reqwest::Client::new();

    let resp = client
        .get(format!("http://127.0.0.1:{}/v1/approvals", port))
        .send()
        .await
        .expect("Request should succeed");

    assert_eq!(resp.status(), 200);
    let body: ListApprovalsResponse = resp.json().await.expect("Should parse");
    assert!(
        body.approvals.is_empty(),
        "Should have no pending approvals"
    );

    handle.abort();
}

#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test approval_api_tests -- --ignored"]
async fn test_approve_approval_with_valid_signature() {
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

    // Queue an approval directly - use test.action which has no automatic execution path
    let queue = carnelian_core::ApprovalQueue::new(pool.clone());
    let approval_id = queue
        .queue_action("test.action", json!({"test": true}), None, None)
        .await
        .expect("Queue should succeed");
    drop(pool);

    let (port, handle, config) = start_db_backed_server(&db_url).await;
    let client = reqwest::Client::new();

    // Generate valid signature using owner signing key
    let signature = config
        .sign_message(approval_id.to_string().as_bytes())
        .expect("Should sign message");
    let signature_hex = hex::encode(signature.to_bytes());

    let resp = client
        .post(format!(
            "http://127.0.0.1:{}/v1/approvals/{}/approve",
            port, approval_id
        ))
        .json(&ApprovalActionRequest {
            signature: signature_hex,
        })
        .send()
        .await
        .expect("Request should succeed");

    assert_eq!(resp.status(), 200);
    let body: ApprovalActionResponse = resp.json().await.expect("Should parse");
    assert_eq!(body.approval_id, approval_id);
    assert_eq!(body.status, "approved");

    handle.abort();
}

#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test approval_api_tests -- --ignored"]
async fn test_deny_approval_with_valid_signature() {
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

    // Use test.action which has no automatic execution path
    let queue = carnelian_core::ApprovalQueue::new(pool.clone());
    let approval_id = queue
        .queue_action("test.action", json!({"key": "test"}), None, None)
        .await
        .expect("Queue should succeed");
    drop(pool);

    let (port, handle, config) = start_db_backed_server(&db_url).await;
    let client = reqwest::Client::new();

    // Generate valid signature using owner signing key
    let signature = config
        .sign_message(approval_id.to_string().as_bytes())
        .expect("Should sign message");
    let signature_hex = hex::encode(signature.to_bytes());

    let resp = client
        .post(format!(
            "http://127.0.0.1:{}/v1/approvals/{}/deny",
            port, approval_id
        ))
        .json(&ApprovalActionRequest {
            signature: signature_hex,
        })
        .send()
        .await
        .expect("Request should succeed");

    assert_eq!(resp.status(), 200);
    let body: ApprovalActionResponse = resp.json().await.expect("Should parse");
    assert_eq!(body.approval_id, approval_id);
    assert_eq!(body.status, "denied");

    handle.abort();
}

#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test approval_api_tests -- --ignored"]
async fn test_batch_approve() {
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

    // Use test.action which has no automatic execution path
    let queue = carnelian_core::ApprovalQueue::new(pool.clone());
    let id1 = queue
        .queue_action("test.action", json!({"cap": "fs.read"}), None, None)
        .await
        .expect("Queue should succeed");
    let id2 = queue
        .queue_action("test.action", json!({"cap": "fs.write"}), None, None)
        .await
        .expect("Queue should succeed");
    drop(pool);

    let (port, handle, config) = start_db_backed_server(&db_url).await;
    let client = reqwest::Client::new();

    // Generate valid signature for batch approval - must use sorted IDs like server expects
    let mut sorted_ids = vec![id1, id2];
    sorted_ids.sort();
    let message = sorted_ids
        .iter()
        .map(|id| id.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let signature = config
        .sign_message(message.as_bytes())
        .expect("Should sign message");
    let signature_hex = hex::encode(signature.to_bytes());

    let resp = client
        .post(format!("http://127.0.0.1:{}/v1/approvals/batch", port))
        .json(&BatchApprovalRequest {
            approval_ids: vec![id1, id2],
            signature: signature_hex,
        })
        .send()
        .await
        .expect("Request should succeed");

    assert_eq!(resp.status(), 200);
    let body: BatchApprovalResponse = resp.json().await.expect("Should parse");
    assert_eq!(body.approved.len(), 2);
    assert!(body.approved.contains(&id1));
    assert!(body.approved.contains(&id2));
    assert!(body.failed.is_empty());

    handle.abort();
}

#[tokio::test]
#[ignore = "Requires Docker - run with: cargo test --test approval_api_tests -- --ignored"]
async fn test_approve_nonexistent_returns_error() {
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

    let (port, handle, config) = start_db_backed_server(&db_url).await;
    let client = reqwest::Client::new();

    let nonexistent_id = Uuid::new_v4();

    // Generate valid signature even for nonexistent approval
    let signature = config
        .sign_message(nonexistent_id.to_string().as_bytes())
        .expect("Should sign message");
    let signature_hex = hex::encode(signature.to_bytes());

    let resp = client
        .post(format!(
            "http://127.0.0.1:{}/v1/approvals/{}/approve",
            port, nonexistent_id
        ))
        .json(&ApprovalActionRequest {
            signature: signature_hex,
        })
        .send()
        .await
        .expect("Request should succeed");

    assert_eq!(resp.status(), 404);

    handle.abort();
}
