//! Centralized API client for communicating with the Carnelian server.
//!
//! All functions use `reqwest::Client` and return `Result<T, String>`
//! where `T` is the deserialized response type from `carnelian_common::types`.

use carnelian_common::types::{
    ApprovalActionRequest, ApprovalActionResponse, BatchApprovalRequest, BatchApprovalResponse,
    CancelTaskRequest, CancelTaskResponse, ChannelDetail, ConfigureVoiceRequest,
    ConfigureVoiceResponse, CreateChannelApiRequest, CreateChannelResponse,
    CreateSubAgentApiRequest, CreateSubAgentResponse, CreateTaskRequest, CreateTaskResponse,
    CreateWorkflowRequest, DetailedHealthResponse, ExecuteWorkflowRequest, GrantCapabilityRequest,
    GrantCapabilityResponse, HeartbeatRecord, HeartbeatStatusResponse, IdentityResponse,
    ListApprovalsResponse, ListCapabilitiesResponse, ListChannelsResponse, ListProvidersResponse,
    ListRunsResponse, ListSkillsResponse, ListSubAgentsResponse, ListTasksResponse,
    ListVoicesResponse, ListWorkflowsResponse, MetricsSnapshot, OllamaStatusResponse,
    PaginatedRunLogsResponse, PairChannelApiRequest, PairChannelResponse, RevokeCapabilityResponse,
    RunDetail, SkillRefreshResponse, SkillToggleResponse, StatusResponse, SubAgentActionResponse,
    SubAgentDetail, TaskDetail, TestVoiceRequest, TestVoiceResponse, TopSkillsResponse,
    UpdateChannelApiRequest, UpdateSubAgentApiRequest, UpdateWorkflowRequest, WorkflowDetail,
    WorkflowExecutionResponse, XpHistoryResponse, XpLeaderboardResponse,
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

/// List runs for a task.
pub async fn list_task_runs(task_id: Uuid) -> Result<Vec<RunDetail>, String> {
    client()
        .get(format!("{API_BASE_URL}/v1/tasks/{task_id}/runs"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<ListRunsResponse>()
        .await
        .map(|r| r.runs)
        .map_err(|e| format!("Parse failed: {e}"))
}

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

// ── Health & Status Operations ───────────────────────────────

/// Get detailed health information.
pub async fn get_detailed_health() -> Result<DetailedHealthResponse, String> {
    client()
        .get(format!("{API_BASE_URL}/v1/health/detailed"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<DetailedHealthResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Get system status (workers, models, queue depth).
pub async fn get_system_status() -> Result<StatusResponse, String> {
    client()
        .get(format!("{API_BASE_URL}/v1/status"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<StatusResponse>()
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

// ── Sub-Agent Operations ──────────────────────────────────

/// List sub-agents, optionally filtered by parent and terminated status.
pub async fn list_sub_agents(
    parent_id: Option<Uuid>,
    include_terminated: bool,
) -> Result<ListSubAgentsResponse, String> {
    let mut url = format!("{API_BASE_URL}/v1/sub-agents?include_terminated={include_terminated}");
    if let Some(pid) = parent_id {
        url.push_str(&format!("&parent_id={pid}"));
    }
    client()
        .get(&url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<ListSubAgentsResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Create a new sub-agent.
pub async fn create_sub_agent(
    request: CreateSubAgentApiRequest,
) -> Result<CreateSubAgentResponse, String> {
    client()
        .post(format!("{API_BASE_URL}/v1/sub-agents"))
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<CreateSubAgentResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Update an existing sub-agent.
pub async fn update_sub_agent(
    sub_agent_id: Uuid,
    request: UpdateSubAgentApiRequest,
) -> Result<SubAgentDetail, String> {
    client()
        .put(format!("{API_BASE_URL}/v1/sub-agents/{sub_agent_id}"))
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<SubAgentDetail>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Delete (soft-terminate) a sub-agent.
pub async fn delete_sub_agent(sub_agent_id: Uuid) -> Result<(), String> {
    let resp = client()
        .delete(format!("{API_BASE_URL}/v1/sub-agents/{sub_agent_id}"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;
    if resp.status().is_success() {
        Ok(())
    } else {
        Err(format!("Delete failed: {}", resp.status()))
    }
}

/// Pause a sub-agent.
pub async fn pause_sub_agent(sub_agent_id: Uuid) -> Result<SubAgentActionResponse, String> {
    client()
        .post(format!("{API_BASE_URL}/v1/sub-agents/{sub_agent_id}/pause"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<SubAgentActionResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Resume a paused sub-agent.
pub async fn resume_sub_agent(sub_agent_id: Uuid) -> Result<SubAgentActionResponse, String> {
    client()
        .post(format!(
            "{API_BASE_URL}/v1/sub-agents/{sub_agent_id}/resume"
        ))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<SubAgentActionResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

// ── Workflow Operations ──────────────────────────────────────

/// List workflows, optionally filtering to enabled-only.
pub async fn list_workflows(enabled_only: bool) -> Result<ListWorkflowsResponse, String> {
    client()
        .get(format!(
            "{API_BASE_URL}/v1/workflows?enabled_only={enabled_only}"
        ))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<ListWorkflowsResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Get a single workflow by ID.
#[allow(dead_code)]
pub async fn get_workflow(workflow_id: Uuid) -> Result<WorkflowDetail, String> {
    client()
        .get(format!("{API_BASE_URL}/v1/workflows/{workflow_id}"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<WorkflowDetail>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Create a new workflow.
pub async fn create_workflow(request: CreateWorkflowRequest) -> Result<WorkflowDetail, String> {
    let resp = client()
        .post(format!("{API_BASE_URL}/v1/workflows"))
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Server error {status}: {text}"));
    }
    resp.json::<WorkflowDetail>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Update an existing workflow.
pub async fn update_workflow(
    workflow_id: Uuid,
    request: UpdateWorkflowRequest,
) -> Result<WorkflowDetail, String> {
    let resp = client()
        .put(format!("{API_BASE_URL}/v1/workflows/{workflow_id}"))
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Server error {status}: {text}"));
    }
    resp.json::<WorkflowDetail>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Delete a workflow.
pub async fn delete_workflow(workflow_id: Uuid) -> Result<(), String> {
    let resp = client()
        .delete(format!("{API_BASE_URL}/v1/workflows/{workflow_id}"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;
    if resp.status().is_success() {
        Ok(())
    } else {
        Err(format!("Delete failed: {}", resp.status()))
    }
}

/// Execute a workflow.
pub async fn execute_workflow(
    workflow_id: Uuid,
    request: ExecuteWorkflowRequest,
) -> Result<WorkflowExecutionResponse, String> {
    let resp = client()
        .post(format!("{API_BASE_URL}/v1/workflows/{workflow_id}/execute"))
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Server error {status}: {text}"));
    }
    resp.json::<WorkflowExecutionResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

// ── Channel Operations ──────────────────────────────────────

/// List channel sessions, optionally filtered by type.
pub async fn list_channels(channel_type: Option<String>) -> Result<ListChannelsResponse, String> {
    let mut url = format!("{API_BASE_URL}/v1/channels");
    if let Some(ref ct) = channel_type {
        url.push_str(&format!("?channel_type={ct}"));
    }
    client()
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<ListChannelsResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Get a single channel session by ID.
#[allow(dead_code)]
pub async fn get_channel(session_id: Uuid) -> Result<ChannelDetail, String> {
    client()
        .get(format!("{API_BASE_URL}/v1/channels/{session_id}"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<ChannelDetail>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Create a new channel session.
pub async fn create_channel(
    request: CreateChannelApiRequest,
) -> Result<CreateChannelResponse, String> {
    let resp = client()
        .post(format!("{API_BASE_URL}/v1/channels"))
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Server error {status}: {text}"));
    }
    resp.json::<CreateChannelResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Update an existing channel session.
pub async fn update_channel(
    session_id: Uuid,
    request: UpdateChannelApiRequest,
) -> Result<ChannelDetail, String> {
    let resp = client()
        .put(format!("{API_BASE_URL}/v1/channels/{session_id}"))
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Server error {status}: {text}"));
    }
    resp.json::<ChannelDetail>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Delete a channel session.
pub async fn delete_channel(session_id: Uuid) -> Result<(), String> {
    let resp = client()
        .delete(format!("{API_BASE_URL}/v1/channels/{session_id}"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;
    if resp.status().is_success() {
        Ok(())
    } else {
        Err(format!("Delete failed: {}", resp.status()))
    }
}

/// Initiate pairing for a channel session.
pub async fn pair_channel(
    session_id: Uuid,
    trust_level: Option<String>,
) -> Result<PairChannelResponse, String> {
    let body = PairChannelApiRequest { trust_level };
    let resp = client()
        .post(format!("{API_BASE_URL}/v1/channels/{session_id}/pair"))
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Server error {status}: {text}"));
    }
    resp.json::<PairChannelResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

// ── XP Operations ──────────────────────────────────────────

/// Get paginated XP history for an agent.
pub async fn get_xp_history(
    identity_id: Uuid,
    page: u32,
    page_size: u32,
) -> Result<XpHistoryResponse, String> {
    client()
        .get(format!(
            "{API_BASE_URL}/v1/xp/agents/{identity_id}/history?page={page}&page_size={page_size}"
        ))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<XpHistoryResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Get the XP leaderboard.
pub async fn get_xp_leaderboard() -> Result<XpLeaderboardResponse, String> {
    client()
        .get(format!("{API_BASE_URL}/v1/xp/leaderboard"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<XpLeaderboardResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Get top skills by XP.
pub async fn get_top_skills(limit: i64) -> Result<TopSkillsResponse, String> {
    client()
        .get(format!("{API_BASE_URL}/v1/xp/skills/top?limit={limit}"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<TopSkillsResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

// ── Voice Operations ───────────────────────────────────────

/// Configure voice settings.
pub async fn configure_voice(
    request: ConfigureVoiceRequest,
) -> Result<ConfigureVoiceResponse, String> {
    let resp = client()
        .post(format!("{API_BASE_URL}/v1/voice/configure"))
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Server error {status}: {text}"));
    }
    resp.json::<ConfigureVoiceResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Test text-to-speech.
pub async fn test_voice(request: TestVoiceRequest) -> Result<TestVoiceResponse, String> {
    let resp = client()
        .post(format!("{API_BASE_URL}/v1/voice/test"))
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?;
    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        return Err(format!("Server error {status}: {text}"));
    }
    resp.json::<TestVoiceResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// List available voices.
pub async fn list_voices() -> Result<ListVoicesResponse, String> {
    client()
        .get(format!("{API_BASE_URL}/v1/voice/voices"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<ListVoicesResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

// ── Ledger Operations ────────────────────────────────────────

use carnelian_common::types::{LedgerVerifyResponse, ListLedgerEventsResponse};

/// List ledger events with optional filters.
pub async fn list_ledger_events(
    limit: i64,
    offset: i64,
    action_type: Option<String>,
    actor_id: Option<String>,
    from_ts: Option<String>,
    to_ts: Option<String>,
) -> Result<ListLedgerEventsResponse, String> {
    let mut url = format!("{API_BASE_URL}/v1/ledger/events?limit={limit}&offset={offset}");
    if let Some(at) = action_type {
        url.push_str(&format!("&action_type={at}"));
    }
    if let Some(aid) = actor_id {
        url.push_str(&format!("&actor_id={aid}"));
    }
    if let Some(from) = from_ts {
        url.push_str(&format!("&from_ts={from}"));
    }
    if let Some(to) = to_ts {
        url.push_str(&format!("&to_ts={to}"));
    }
    client()
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<ListLedgerEventsResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Verify ledger chain integrity.
pub async fn verify_ledger_chain() -> Result<LedgerVerifyResponse, String> {
    client()
        .get(format!("{API_BASE_URL}/v1/ledger/verify"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<LedgerVerifyResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

// ── Setup Status Operations ────────────────────────────────

use carnelian_common::types::{SetupCompleteResponse, SetupStatusResponse};

/// Get setup status.
pub async fn get_setup_status() -> Result<SetupStatusResponse, String> {
    client()
        .get(format!("{API_BASE_URL}/v1/config/setup-status"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<SetupStatusResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Mark setup as complete.
pub async fn mark_setup_complete() -> Result<SetupCompleteResponse, String> {
    client()
        .post(format!("{API_BASE_URL}/v1/config/setup-complete"))
        .json(&serde_json::json!({}))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<SetupCompleteResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

// ── Skill Book Operations ─────────────────────────────────

use carnelian_common::types::{
    ActivateSkillRequest, ActivateSkillResponse, DeactivateSkillResponse, SkillBookCatalog,
};

/// List all skills in the Skill Book catalog.
pub async fn list_skill_book() -> Result<SkillBookCatalog, String> {
    client()
        .get(format!("{API_BASE_URL}/v1/node-registry"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<SkillBookCatalog>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Activate a skill from the Skill Book.
pub async fn activate_skill(
    skill_id: &str,
    config: std::collections::HashMap<String, String>,
) -> Result<ActivateSkillResponse, String> {
    let request = ActivateSkillRequest { config };
    client()
        .post(format!(
            "{API_BASE_URL}/v1/node-registry/{skill_id}/activate"
        ))
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<ActivateSkillResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Deactivate a skill.
pub async fn deactivate_skill(skill_id: &str) -> Result<DeactivateSkillResponse, String> {
    client()
        .delete(format!(
            "{API_BASE_URL}/v1/node-registry/{skill_id}/deactivate"
        ))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<DeactivateSkillResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

// ── Elixir Operations ─────────────────────────────────

use carnelian_common::types::{
    ApproveDraftResponse, CreateElixirRequest, ElixirDetail, ElixirSearchResponse,
    ListElixirDraftsResponse, ListElixirsQuery, ListElixirsResponse, RejectDraftResponse,
};

/// List elixirs with optional filtering and pagination.
pub async fn elixirs_list(filters: ListElixirsQuery) -> Result<ListElixirsResponse, String> {
    let mut url = format!(
        "{API_BASE_URL}/v1/elixirs?page={}&page_size={}",
        filters.page, filters.page_size
    );
    if let Some(elixir_type) = filters.elixir_type {
        url.push_str(&format!("&elixir_type={elixir_type}"));
    }
    if let Some(skill_id) = filters.skill_id {
        url.push_str(&format!("&skill_id={skill_id}"));
    }
    if let Some(active) = filters.active {
        url.push_str(&format!("&active={active}"));
    }
    client()
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<ListElixirsResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Create a new elixir.
pub async fn elixirs_create(request: CreateElixirRequest) -> Result<ElixirDetail, String> {
    client()
        .post(format!("{API_BASE_URL}/v1/elixirs"))
        .json(&request)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<ElixirDetail>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Search elixirs using semantic search.
pub async fn elixirs_search(query: String, limit: u32) -> Result<ElixirSearchResponse, String> {
    let url = reqwest::Url::parse_with_params(
        &format!("{API_BASE_URL}/v1/elixirs/search"),
        &[("q", query.as_str()), ("limit", &limit.to_string())],
    )
    .map_err(|e| format!("URL parse failed: {e}"))?;
    client()
        .get(url)
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<ElixirSearchResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// List all elixir drafts.
pub async fn elixirs_drafts_list() -> Result<ListElixirDraftsResponse, String> {
    client()
        .get(format!("{API_BASE_URL}/v1/elixirs/drafts"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<ListElixirDraftsResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Approve an elixir draft and promote it to an elixir.
pub async fn elixirs_draft_approve(draft_id: Uuid) -> Result<ApproveDraftResponse, String> {
    client()
        .post(format!(
            "{API_BASE_URL}/v1/elixirs/drafts/{draft_id}/approve"
        ))
        .json(&serde_json::json!({}))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<ApproveDraftResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Reject an elixir draft.
pub async fn elixirs_draft_reject(draft_id: Uuid) -> Result<RejectDraftResponse, String> {
    client()
        .post(format!(
            "{API_BASE_URL}/v1/elixirs/drafts/{draft_id}/reject"
        ))
        .json(&serde_json::json!({}))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<RejectDraftResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

// ── MAGIC Operations ──────────────────────────────────

use carnelian_common::types::{
    AddMantraEntryRequest, EntropyLogResponse, EntropySampleRequest, EntropySampleResponse,
    ListMantraCategoriesResponse, ListMantraEntriesResponse, MagicAuthStatusResponse,
    MagicConfigResponse, MagicConfigUpdateRequest, MagicElixirsRehashResponse,
    MantraEntryDetail, MantraHistoryResponse, MantraSimulateResponse, QuantinuumLoginRequest,
    QuantinuumLoginResponse, QuantinuumRefreshResponse, UpdateMantraEntryRequest,
};

// ── Entropy functions ──

/// Get entropy health status.
pub async fn magic_entropy_health() -> Result<serde_json::Value, String> {
    client()
        .get(format!("{API_BASE_URL}/v1/magic/entropy/health"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<serde_json::Value>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Sample entropy bytes.
pub async fn magic_entropy_sample(
    bytes: usize,
    provider: Option<String>,
) -> Result<EntropySampleResponse, String> {
    client()
        .post(format!("{API_BASE_URL}/v1/magic/entropy/sample"))
        .json(&EntropySampleRequest { bytes, provider })
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<EntropySampleResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Get entropy log entries.
#[allow(dead_code)]
pub async fn magic_entropy_log(limit: i64) -> Result<EntropyLogResponse, String> {
    client()
        .get(format!("{API_BASE_URL}/v1/magic/entropy/log?limit={limit}"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<EntropyLogResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

// ── Mantra functions ──

/// List all mantra categories.
pub async fn magic_mantras_list() -> Result<ListMantraCategoriesResponse, String> {
    client()
        .get(format!("{API_BASE_URL}/v1/magic/mantras"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<ListMantraCategoriesResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// List mantra entries for a category.
pub async fn magic_mantras_by_category(
    category_id: Uuid,
) -> Result<ListMantraEntriesResponse, String> {
    client()
        .get(format!("{API_BASE_URL}/v1/magic/mantras/{category_id}"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<ListMantraEntriesResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Add a new mantra entry.
pub async fn magic_mantra_add(
    category_id: Uuid,
    text: String,
    elixir_id: Option<Uuid>,
) -> Result<MantraEntryDetail, String> {
    client()
        .post(format!(
            "{API_BASE_URL}/v1/magic/mantras/categories/{category_id}/entries"
        ))
        .json(&AddMantraEntryRequest { text, elixir_id })
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<MantraEntryDetail>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Update a mantra entry.
pub async fn magic_mantra_update(
    entry_id: Uuid,
    text: Option<String>,
    enabled: Option<bool>,
    elixir_id: Option<Option<Uuid>>,
) -> Result<MantraEntryDetail, String> {
    client()
        .put(format!(
            "{API_BASE_URL}/v1/magic/mantras/entries/{entry_id}"
        ))
        .json(&UpdateMantraEntryRequest {
            text,
            enabled,
            elixir_id,
        })
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<MantraEntryDetail>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Get mantra selection history.
pub async fn magic_mantra_history() -> Result<MantraHistoryResponse, String> {
    client()
        .get(format!("{API_BASE_URL}/v1/magic/mantras/history"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<MantraHistoryResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Simulate a mantra selection.
pub async fn magic_mantra_simulate() -> Result<MantraSimulateResponse, String> {
    client()
        .post(format!("{API_BASE_URL}/v1/magic/mantras/simulate"))
        .json(&serde_json::json!({}))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<MantraSimulateResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

// ── Auth & Config functions ──

/// Get MAGIC configuration.
pub async fn magic_get_config() -> Result<MagicConfigResponse, String> {
    client()
        .get(format!("{API_BASE_URL}/v1/magic/config"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<MagicConfigResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Update MAGIC configuration.
pub async fn magic_update_config(
    quantum_origin_api_key: Option<String>,
    quantinuum_enabled: Option<bool>,
    qiskit_enabled: Option<bool>,
) -> Result<serde_json::Value, String> {
    client()
        .post(format!("{API_BASE_URL}/v1/magic/config"))
        .json(&MagicConfigUpdateRequest {
            quantum_origin_api_key,
            quantinuum_enabled,
            qiskit_enabled,
        })
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<serde_json::Value>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Login to Quantinuum.
pub async fn magic_quantinuum_login(
    email: String,
    password: String,
) -> Result<QuantinuumLoginResponse, String> {
    client()
        .post(format!(
            "{API_BASE_URL}/v1/magic/auth/quantinuum/login"
        ))
        .json(&QuantinuumLoginRequest { email, password })
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<QuantinuumLoginResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Refresh Quantinuum token.
pub async fn magic_quantinuum_refresh() -> Result<QuantinuumRefreshResponse, String> {
    client()
        .post(format!(
            "{API_BASE_URL}/v1/magic/auth/quantinuum/refresh"
        ))
        .json(&serde_json::json!({}))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<QuantinuumRefreshResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

/// Get MAGIC authentication status.
pub async fn magic_auth_status() -> Result<MagicAuthStatusResponse, String> {
    client()
        .get(format!("{API_BASE_URL}/v1/magic/auth/status"))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<MagicAuthStatusResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}

// ── Elixir MAGIC function ──

/// Rehash all elixirs with quantum entropy.
#[allow(dead_code)]
pub async fn magic_elixirs_rehash() -> Result<MagicElixirsRehashResponse, String> {
    client()
        .post(format!("{API_BASE_URL}/v1/magic/elixirs/rehash"))
        .json(&serde_json::json!({}))
        .send()
        .await
        .map_err(|e| format!("Request failed: {e}"))?
        .json::<MagicElixirsRehashResponse>()
        .await
        .map_err(|e| format!("Parse failed: {e}"))
}
