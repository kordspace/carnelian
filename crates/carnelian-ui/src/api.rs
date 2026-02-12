//! Centralized API client for communicating with the Carnelian server.
//!
//! All functions use `reqwest::Client` and return `Result<T, String>`
//! where `T` is the deserialized response type from `carnelian_common::types`.

use carnelian_common::types::{
    CancelTaskRequest, CancelTaskResponse, CreateTaskRequest, CreateTaskResponse,
    ListSkillsResponse, ListTasksResponse, MetricsSnapshot, PaginatedRunLogsResponse, RunDetail,
    SkillRefreshResponse, SkillToggleResponse, TaskDetail,
};
use uuid::Uuid;

/// Base URL for the Carnelian server REST API.
const API_BASE_URL: &str = "http://localhost:18789";

/// Shared HTTP client (created once per call; callers may cache externally).
fn client() -> reqwest::Client {
    reqwest::Client::new()
}

// ── Task Operations ─────────────────────────────────────────

/// Create a new task.
pub async fn create_task(
    title: String,
    description: Option<String>,
    skill_id: Option<Uuid>,
    priority: i32,
) -> Result<CreateTaskResponse, String> {
    let body = CreateTaskRequest {
        title,
        description,
        skill_id,
        priority,
        requires_approval: false,
    };
    client()
        .post(format!("{API_BASE_URL}/v1/tasks"))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<CreateTaskResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// List all tasks.
pub async fn list_tasks() -> Result<ListTasksResponse, String> {
    client()
        .get(format!("{API_BASE_URL}/v1/tasks"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<ListTasksResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Get a single task by ID.
#[allow(dead_code)]
pub async fn get_task(task_id: Uuid) -> Result<TaskDetail, String> {
    client()
        .get(format!("{API_BASE_URL}/v1/tasks/{task_id}"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<TaskDetail>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Cancel a task.
pub async fn cancel_task(task_id: Uuid, reason: String) -> Result<CancelTaskResponse, String> {
    let body = CancelTaskRequest { reason };
    client()
        .post(format!("{API_BASE_URL}/v1/tasks/{task_id}/cancel"))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<CancelTaskResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

// ── Run Operations ──────────────────────────────────────────

/// Get a single run by ID.
#[allow(dead_code)]
pub async fn get_run(run_id: Uuid) -> Result<RunDetail, String> {
    client()
        .get(format!("{API_BASE_URL}/v1/runs/{run_id}"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<RunDetail>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Get paginated logs for a run.
#[allow(dead_code)]
pub async fn get_run_logs(
    run_id: Uuid,
    page: u32,
    page_size: u32,
) -> Result<PaginatedRunLogsResponse, String> {
    client()
        .get(format!(
            "{API_BASE_URL}/v1/runs/{run_id}/logs?page={page}&page_size={page_size}"
        ))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<PaginatedRunLogsResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

// ── Skill Operations ────────────────────────────────────────

/// List all skills.
pub async fn list_skills() -> Result<ListSkillsResponse, String> {
    client()
        .get(format!("{API_BASE_URL}/v1/skills"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<ListSkillsResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Enable a skill.
pub async fn enable_skill(skill_id: Uuid) -> Result<SkillToggleResponse, String> {
    client()
        .post(format!("{API_BASE_URL}/v1/skills/{skill_id}/enable"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<SkillToggleResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Disable a skill.
pub async fn disable_skill(skill_id: Uuid) -> Result<SkillToggleResponse, String> {
    client()
        .post(format!("{API_BASE_URL}/v1/skills/{skill_id}/disable"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<SkillToggleResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Refresh the skill registry (scan for new/updated/removed skills).
pub async fn refresh_skills() -> Result<SkillRefreshResponse, String> {
    client()
        .post(format!("{API_BASE_URL}/v1/skills/refresh"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<SkillRefreshResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

// ── Metrics Operations ────────────────────────────────────

/// Fetch aggregated performance metrics.
pub async fn get_metrics() -> Result<MetricsSnapshot, String> {
    client()
        .get(format!("{API_BASE_URL}/v1/metrics"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<MetricsSnapshot>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}
