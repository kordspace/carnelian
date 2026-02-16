//! Centralized API client for communicating with the Carnelian server.
//!
//! All functions use `reqwest::Client` and return `Result<T, String>`
//! where `T` is the deserialized response type from `carnelian_common::types`.

use carnelian_common::types::{
    ApprovalActionRequest, ApprovalActionResponse, BatchApprovalRequest, BatchApprovalResponse,
    CancelTaskRequest, CancelTaskResponse, CreateTaskRequest, CreateTaskResponse,
    GrantCapabilityRequest, GrantCapabilityResponse, HeartbeatRecord, HeartbeatStatusResponse,
    IdentityResponse, ListApprovalsResponse, ListCapabilitiesResponse, ListProvidersResponse,
    ListSkillsResponse, ListTasksResponse, MetricsSnapshot, OllamaStatusResponse,
    PaginatedRunLogsResponse, RevokeCapabilityResponse, RunDetail, SkillRefreshResponse,
    SkillToggleResponse, TaskDetail,
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

// ── Approval Operations ────────────────────────────────────

/// List pending approvals.
pub async fn list_pending_approvals(limit: i64) -> Result<ListApprovalsResponse, String> {
    client()
        .get(format!("{API_BASE_URL}/v1/approvals?limit={limit}"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<ListApprovalsResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Approve an approval request.
pub async fn approve_approval(
    approval_id: Uuid,
    signature: String,
) -> Result<ApprovalActionResponse, String> {
    let body = ApprovalActionRequest { signature };
    let resp = client()
        .post(format!("{API_BASE_URL}/v1/approvals/{approval_id}/approve"))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Server error {status}: {text}"));
    }
    resp.json::<ApprovalActionResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Deny an approval request.
pub async fn deny_approval(
    approval_id: Uuid,
    signature: String,
) -> Result<ApprovalActionResponse, String> {
    let body = ApprovalActionRequest { signature };
    let resp = client()
        .post(format!("{API_BASE_URL}/v1/approvals/{approval_id}/deny"))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Server error {status}: {text}"));
    }
    resp.json::<ApprovalActionResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Batch approve multiple approval requests.
pub async fn batch_approve_approvals(
    approval_ids: Vec<Uuid>,
    signature: String,
) -> Result<BatchApprovalResponse, String> {
    let body = BatchApprovalRequest {
        approval_ids,
        signature,
    };
    let resp = client()
        .post(format!("{API_BASE_URL}/v1/approvals/batch"))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Server error {status}: {text}"));
    }
    resp.json::<BatchApprovalResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

// ── Capability Operations ──────────────────────────────────

/// List capability grants with optional filters.
pub async fn list_capabilities(
    subject_type: Option<String>,
    subject_id: Option<String>,
) -> Result<ListCapabilitiesResponse, String> {
    let mut url = format!("{API_BASE_URL}/v1/capabilities");
    let mut params = Vec::new();
    if let Some(ref st) = subject_type {
        params.push(format!("subject_type={st}"));
    }
    if let Some(ref si) = subject_id {
        params.push(format!("subject_id={si}"));
    }
    if !params.is_empty() {
        url.push('?');
        url.push_str(&params.join("&"));
    }
    client()
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<ListCapabilitiesResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Grant a capability.
pub async fn grant_capability(
    request: GrantCapabilityRequest,
) -> Result<GrantCapabilityResponse, String> {
    client()
        .post(format!("{API_BASE_URL}/v1/capabilities"))
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<GrantCapabilityResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Revoke a capability grant.
pub async fn revoke_capability(grant_id: Uuid) -> Result<RevokeCapabilityResponse, String> {
    client()
        .delete(format!("{API_BASE_URL}/v1/capabilities/{grant_id}"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<RevokeCapabilityResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

// ── Heartbeat Operations ────────────────────────────────────

/// Get recent heartbeat records.
pub async fn get_recent_heartbeats(limit: i64) -> Result<Vec<HeartbeatRecord>, String> {
    client()
        .get(format!("{API_BASE_URL}/v1/heartbeats?limit={limit}"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<Vec<HeartbeatRecord>>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Get current heartbeat status (mantra, last/next times).
pub async fn get_heartbeat_status() -> Result<HeartbeatStatusResponse, String> {
    client()
        .get(format!("{API_BASE_URL}/v1/heartbeats/status"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<HeartbeatStatusResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

// ── Identity Operations ─────────────────────────────────────

/// Get core identity information.
pub async fn get_identity() -> Result<IdentityResponse, String> {
    client()
        .get(format!("{API_BASE_URL}/v1/identity"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<IdentityResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Get full SOUL.md content as plain text.
pub async fn get_soul_content() -> Result<String, String> {
    client()
        .get(format!("{API_BASE_URL}/v1/identity/soul"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .text()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

// ── Provider Operations ─────────────────────────────────────

/// List all model providers.
pub async fn list_providers() -> Result<ListProvidersResponse, String> {
    client()
        .get(format!("{API_BASE_URL}/v1/providers"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<ListProvidersResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Get Ollama connection status and available models.
pub async fn get_ollama_status() -> Result<OllamaStatusResponse, String> {
    client()
        .get(format!("{API_BASE_URL}/v1/providers/ollama/status"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<OllamaStatusResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}
