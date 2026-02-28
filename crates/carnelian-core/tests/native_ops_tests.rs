use carnelian_common::types::{InvokeRequest, InvokeStatus, RunId};
use carnelian_core::worker::{NativeWorkerTransport, WorkerTransport};
use carnelian_core::{Config, EventStream};
use serde_json::json;
use std::io::Write;
use std::sync::Arc;
use tempfile::NamedTempFile;

// Shared helpers

fn make_transport() -> NativeWorkerTransport {
    NativeWorkerTransport::new(
        "test-native".into(),
        Arc::new(EventStream::new(100, 10)),
        Arc::new(Config::default()),
    )
}

fn make_request(skill_name: &str, input: serde_json::Value) -> InvokeRequest {
    InvokeRequest {
        run_id: RunId::new(),
        skill_name: skill_name.into(),
        input,
        timeout_secs: 10,
        correlation_id: None,
    }
}

// Git Operations (git.read capability)

#[tokio::test]
async fn test_git_status() {
    let transport = make_transport();
    let request = make_request(
        "git_status",
        json!({"path": ".", "capabilities": ["git.read"]}),
    );
    let response = transport.invoke(request).await.unwrap();

    assert_eq!(response.status, InvokeStatus::Success);
    assert!(response.result["output"].is_string());
}

#[tokio::test]
async fn test_git_diff() {
    let transport = make_transport();
    let request = make_request(
        "git_diff",
        json!({"path": ".", "staged": false, "capabilities": ["git.read"]}),
    );
    let response = transport.invoke(request).await.unwrap();

    assert_eq!(response.status, InvokeStatus::Success);
    assert!(response.result["output"].is_string());
}

#[tokio::test]
async fn test_git_log() {
    let transport = make_transport();
    let request = make_request(
        "git_log",
        json!({"path": ".", "max_count": 5, "oneline": true, "capabilities": ["git.read"]}),
    );
    let response = transport.invoke(request).await.unwrap();

    assert_eq!(response.status, InvokeStatus::Success);
    assert!(response.result["output"].is_string());
}

#[tokio::test]
async fn test_git_branch() {
    let transport = make_transport();
    let request = make_request(
        "git_branch",
        json!({"path": ".", "capabilities": ["git.read"]}),
    );
    let response = transport.invoke(request).await.unwrap();

    assert_eq!(response.status, InvokeStatus::Success);
    assert!(response.result["output"].is_string());
}

// Filesystem Operations (fs.read capability)

#[tokio::test]
async fn test_dir_list() {
    let transport = make_transport();
    let request = make_request(
        "dir_list",
        json!({"path": ".", "depth": 1, "capabilities": ["fs.read"]}),
    );
    let response = transport.invoke(request).await.unwrap();

    assert_eq!(response.status, InvokeStatus::Success);
    assert!(response.result["entries"].is_array());
}

#[tokio::test]
async fn test_file_read() {
    let transport = make_transport();
    let request = make_request(
        "file_read",
        json!({"path": "Cargo.toml", "capabilities": ["fs.read"]}),
    );
    let response = transport.invoke(request).await.unwrap();

    assert_eq!(response.status, InvokeStatus::Success);
    assert!(response.result["content"].is_string());
    assert_eq!(response.result["truncated"], false);
}

#[tokio::test]
async fn test_file_hash() {
    let transport = make_transport();
    let request = make_request(
        "file_hash",
        json!({"path": "Cargo.toml", "capabilities": ["fs.read"]}),
    );
    let response = transport.invoke(request).await.unwrap();

    assert_eq!(response.status, InvokeStatus::Success);
    assert!(response.result["hash"].is_string());
}

#[tokio::test]
async fn test_file_search() {
    // Check if ripgrep is installed
    let rg_check = tokio::process::Command::new("rg")
        .arg("--version")
        .output()
        .await;

    if rg_check.is_err() {
        eprintln!("Skipping test_file_search: ripgrep (rg) not installed");
        return;
    }

    let transport = make_transport();
    let request = make_request(
        "file_search",
        json!({"pattern": "carnelian", "path": ".", "capabilities": ["fs.read"]}),
    );
    let response = transport.invoke(request).await.unwrap();

    assert_eq!(response.status, InvokeStatus::Success);
    assert!(response.result["output"].is_string());
}

// Filesystem Write Operations (fs.write capability)

#[tokio::test]
async fn test_file_move() {
    let transport = make_transport();

    let mut tmp_src = NamedTempFile::new().unwrap();
    writeln!(tmp_src, "test content").unwrap();
    let src_path = tmp_src.path().to_str().unwrap().to_string();

    let tmp_dst = NamedTempFile::new().unwrap();
    let dst_path = tmp_dst.path().to_str().unwrap().to_string();

    let request = make_request(
        "file_move",
        json!({"src": src_path, "dst": dst_path, "capabilities": ["fs.write"]}),
    );
    let response = transport.invoke(request).await.unwrap();

    assert_eq!(response.status, InvokeStatus::Success);
    assert_eq!(response.result["moved"], true);
}

#[tokio::test]
#[ignore = "requires owner approval"]
async fn test_file_write() {
    let transport = make_transport();

    let tmp = NamedTempFile::new().unwrap();
    let tmp_path = tmp.path().to_str().unwrap().to_string();

    let request = make_request(
        "file_write",
        json!({
            "path": tmp_path,
            "content": "test",
            "capabilities": ["fs.write"],
            "_approval_signature": "test_signature"
        }),
    );
    let response = transport.invoke(request).await.unwrap();

    assert_eq!(response.status, InvokeStatus::Success);
    assert_eq!(response.result["written"], true);
}

#[tokio::test]
#[ignore = "requires owner approval"]
async fn test_file_delete() {
    let transport = make_transport();

    let mut tmp = NamedTempFile::new().unwrap();
    writeln!(tmp, "test content").unwrap();
    let tmp_path = tmp.path().to_str().unwrap().to_string();

    let request = make_request(
        "file_delete",
        json!({
            "path": tmp_path,
            "capabilities": ["fs.write"],
            "_approval_signature": "test_signature"
        }),
    );
    let response = transport.invoke(request).await.unwrap();

    assert_eq!(response.status, InvokeStatus::Success);
    assert_eq!(response.result["deleted"], true);
}

#[tokio::test]
#[ignore = "requires owner approval"]
async fn test_git_commit() {
    let transport = make_transport();
    let request = make_request(
        "git_commit",
        json!({
            "path": ".",
            "message": "test",
            "capabilities": ["git.write"],
            "_approval_signature": "test_signature"
        }),
    );
    let response = transport.invoke(request).await.unwrap();

    assert_eq!(response.status, InvokeStatus::Success);
    assert!(response.result["output"].is_string());
}

// Docker Operations (docker.read capability)

#[tokio::test]
async fn test_docker_ps() {
    let transport = make_transport();
    let request = make_request("docker_ps", json!({"capabilities": ["docker.read"]}));
    let response = transport.invoke(request).await.unwrap();

    assert_eq!(response.status, InvokeStatus::Success);
    assert!(response.result["containers"].is_array());
}

#[tokio::test]
async fn test_docker_logs() {
    let transport = make_transport();
    let request = make_request(
        "docker_logs",
        json!({"container_id": "test", "tail": "10", "capabilities": ["docker.read"]}),
    );
    let response = transport.invoke(request).await.unwrap();

    // Should handle missing container gracefully (404 error wrapped in Failed status)
    assert_eq!(response.status, InvokeStatus::Failed);
    assert!(response.error.is_some());
}

#[tokio::test]
async fn test_docker_stats() {
    let transport = make_transport();
    let request = make_request(
        "docker_stats",
        json!({"container_id": "test", "capabilities": ["docker.read"]}),
    );
    let response = transport.invoke(request).await.unwrap();

    // Success or Failed (no live container) are both acceptable
    assert!(response.status == InvokeStatus::Success || response.status == InvokeStatus::Failed);
}

#[tokio::test]
#[ignore = "requires owner approval"]
async fn test_docker_exec() {
    let transport = make_transport();
    let request = make_request(
        "docker_exec",
        json!({
            "container_id": "test",
            "cmd": ["echo", "hi"],
            "capabilities": ["docker.exec"],
            "_approval_signature": "test_signature"
        }),
    );
    let response = transport.invoke(request).await.unwrap();

    assert_eq!(response.status, InvokeStatus::Success);
    assert!(response.result["output"].is_string());
}

// System Operations (system.read capability)

#[tokio::test]
async fn test_process_list() {
    let transport = make_transport();
    let request = make_request("process_list", json!({"capabilities": ["system.read"]}));
    let response = transport.invoke(request).await.unwrap();

    assert_eq!(response.status, InvokeStatus::Success);
    assert!(response.result["processes"].is_array());
}

#[tokio::test]
async fn test_disk_usage() {
    let transport = make_transport();
    let request = make_request("disk_usage", json!({"capabilities": ["system.read"]}));
    let response = transport.invoke(request).await.unwrap();

    assert_eq!(response.status, InvokeStatus::Success);
    assert!(response.result["disks"].is_array());
}

#[tokio::test]
async fn test_network_stats() {
    let transport = make_transport();
    let request = make_request("network_stats", json!({"capabilities": ["system.read"]}));
    let response = transport.invoke(request).await.unwrap();

    assert_eq!(response.status, InvokeStatus::Success);
    assert!(response.result["networks"].is_array());
}

// Environment Operations (env.read capability)

#[tokio::test]
async fn test_env_get() {
    let transport = make_transport();
    let request = make_request(
        "env_get",
        json!({"key": "PATH", "capabilities": ["env.read"]}),
    );
    let response = transport.invoke(request).await.unwrap();

    assert_eq!(response.status, InvokeStatus::Success);
    assert_eq!(response.result["key"], "PATH");
    assert!(response.result["value"].is_string());
}

// Negative Capability Tests

#[tokio::test]
async fn test_git_status_missing_capability() {
    let transport = make_transport();
    let request = make_request("git_status", json!({"path": "."}));
    let response = transport.invoke(request).await.unwrap();

    assert_eq!(response.status, InvokeStatus::Failed);
    assert!(response.error.is_some());
    assert!(response.error.unwrap().contains("git.read"));
}

#[tokio::test]
async fn test_file_read_missing_capability() {
    let transport = make_transport();
    let request = make_request("file_read", json!({"path": "Cargo.toml"}));
    let response = transport.invoke(request).await.unwrap();

    assert_eq!(response.status, InvokeStatus::Failed);
    assert!(response.error.is_some());
    assert!(response.error.unwrap().contains("fs.read"));
}

#[tokio::test]
async fn test_env_get_disallowed_key() {
    let transport = make_transport();
    let request = make_request(
        "env_get",
        json!({"key": "SECRET_KEY", "capabilities": ["env.read"]}),
    );
    let response = transport.invoke(request).await.unwrap();

    assert_eq!(response.status, InvokeStatus::Failed);
    assert!(response.error.is_some());
    assert!(response.error.unwrap().contains("not in the allowed list"));
}
