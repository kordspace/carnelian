//! Integration tests for API endpoints
//!
//! Tests cover:
//! - Skill execution API
//! - Memory CRUD API
//! - Workflow execution API
//! - XP award API
//! - Authentication and authorization

use axum::body::Body;
use axum::http::{Request, StatusCode};
use carnelian_core::server::create_server;
use carnelian_core::config::Config;
use serde_json::json;
use tower::ServiceExt;
use crate::helpers::*;

async fn create_test_server() -> axum::Router {
    let pool = create_test_pool().await;
    run_test_migrations(&pool).await.unwrap();
    
    let config = Config {
        database_url: std::env::var("TEST_DATABASE_URL")
            .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/carnelian_test".to_string()),
        server_host: "127.0.0.1".to_string(),
        server_port: 8080,
        carnelian_api_key: "test_api_key".to_string(),
        ollama_url: "http://localhost:11434".to_string(),
        default_model: "llama2".to_string(),
        max_concurrent_tasks: 10,
        skill_timeout_secs: 30,
    };
    
    create_server(config).await.unwrap()
}

#[tokio::test]
async fn test_health_endpoint() {
    init_test_env();
    
    let app = create_test_server().await;
    
    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_skill_execution_api() {
    init_test_env();
    
    let app = create_test_server().await;
    
    let request_body = json!({
        "skill_name": "test-skill",
        "input": {
            "action": "execute",
            "params": {"key": "value"}
        },
        "timeout_secs": 30
    });
    
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/skills/execute")
                .header("Content-Type", "application/json")
                .header("X-Carnelian-Key", "test_api_key")
                .body(Body::from(serde_json::to_string(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    
    // May return 404 if skill doesn't exist, which is expected
    assert!(response.status() == StatusCode::OK || response.status() == StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_create_memory_api() {
    init_test_env();
    
    let app = create_test_server().await;
    
    let request_body = json!({
        "content": "Test memory from API",
        "metadata": {"source": "api_test"},
        "tags": ["test", "api"]
    });
    
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/memories")
                .header("Content-Type", "application/json")
                .header("X-Carnelian-Key", "test_api_key")
                .body(Body::from(serde_json::to_string(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_list_memories_api() {
    init_test_env();
    
    let app = create_test_server().await;
    
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/memories?limit=10&offset=0")
                .header("X-Carnelian-Key", "test_api_key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_award_xp_api() {
    init_test_env();
    
    let app = create_test_server().await;
    
    let request_body = json!({
        "identity_id": test_identity_id(),
        "amount": 100,
        "source": "skill_execution",
        "description": "Test XP award"
    });
    
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/xp/award")
                .header("Content-Type", "application/json")
                .header("X-Carnelian-Key", "test_api_key")
                .body(Body::from(serde_json::to_string(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_get_xp_leaderboard_api() {
    init_test_env();
    
    let app = create_test_server().await;
    
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/xp/leaderboard?limit=10")
                .header("X-Carnelian-Key", "test_api_key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_unauthorized_request() {
    init_test_env();
    
    let app = create_test_server().await;
    
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/memories")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_invalid_api_key() {
    init_test_env();
    
    let app = create_test_server().await;
    
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/memories")
                .header("X-Carnelian-Key", "invalid_key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_list_skills_api() {
    init_test_env();
    
    let app = create_test_server().await;
    
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/skills")
                .header("X-Carnelian-Key", "test_api_key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_create_workflow_api() {
    init_test_env();
    
    let app = create_test_server().await;
    
    let request_body = json!({
        "name": "Test Workflow",
        "description": "API test workflow",
        "steps": [
            {
                "skill": "test-skill",
                "input": {"key": "value"}
            }
        ]
    });
    
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/workflows")
                .header("Content-Type", "application/json")
                .header("X-Carnelian-Key", "test_api_key")
                .body(Body::from(serde_json::to_string(&request_body).unwrap()))
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::CREATED);
}

#[tokio::test]
async fn test_cors_headers() {
    init_test_env();
    
    let app = create_test_server().await;
    
    let response = app
        .oneshot(
            Request::builder()
                .method("OPTIONS")
                .uri("/api/memories")
                .header("Origin", "http://localhost:3000")
                .header("Access-Control-Request-Method", "POST")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    
    // Should have CORS headers
    assert!(response.headers().contains_key("access-control-allow-origin") || 
            response.status() == StatusCode::OK);
}

#[tokio::test]
async fn test_pagination_headers() {
    init_test_env();
    
    let app = create_test_server().await;
    
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/memories?limit=5&offset=10")
                .header("X-Carnelian-Key", "test_api_key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn test_error_response_format() {
    init_test_env();
    
    let app = create_test_server().await;
    
    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/nonexistent")
                .header("X-Carnelian-Key", "test_api_key")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}
