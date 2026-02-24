//! Workflow execution engine for Carnelian OS
//!
//! This module provides database-backed workflow definitions, validation,
//! execution with dependency resolution, automatic skill chaining, and
//! integration with the Scheduler for task dispatching.
//!
//! # Architecture
//!
//! ```text
//! WorkflowEngine → workflows table (skill_chain JSONB)
//!        ↓
//! Validation (DAG check, skill existence, input mapping)
//!        ↓
//! Topological Sort → Execution Order
//!        ↓
//! Step Execution → Scheduler → WorkerManager → Skills
//!        ↓
//! EventStream (lifecycle events)
//!        ↓
//! task_runs (execution history with workflow metadata)
//! ```

use std::collections::{HashMap, HashSet, VecDeque};
use std::sync::Arc;
use std::time::Instant;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use carnelian_common::types::{
    EventEnvelope, EventLevel, EventType, StepResultDetail, WorkflowDetail,
    WorkflowExecutionResponse, WorkflowStepDef,
};
use carnelian_common::{Error, Result};

use crate::events::EventStream;

// =============================================================================
// TYPES
// =============================================================================

/// Internal workflow definition loaded from the database.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowDefinition {
    pub workflow_id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub created_by: Option<Uuid>,
    pub steps: Vec<WorkflowStepDef>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

impl WorkflowDefinition {
    /// Convert to the API detail type.
    #[must_use]
    pub fn to_detail(&self) -> WorkflowDetail {
        WorkflowDetail {
            workflow_id: self.workflow_id,
            name: self.name.clone(),
            description: self.description.clone(),
            created_by: self.created_by,
            steps: self.steps.clone(),
            enabled: self.enabled,
            created_at: self.created_at,
            updated_at: self.updated_at,
        }
    }
}

/// Execution context that holds step outputs and shared variables.
#[derive(Debug, Clone, Default)]
pub struct WorkflowExecutionContext {
    /// The original workflow input.
    pub input: JsonValue,
    /// Outputs keyed by step_id.
    pub step_outputs: HashMap<String, JsonValue>,
    /// Shared context variables that steps can read/write.
    pub variables: HashMap<String, JsonValue>,
}

/// Overall result of a workflow execution.
#[derive(Debug, Clone)]
pub enum WorkflowExecutionResult {
    Success {
        steps: Vec<StepResult>,
        total_duration_ms: u64,
    },
    PartialSuccess {
        steps: Vec<StepResult>,
        total_duration_ms: u64,
        message: String,
    },
    Failed {
        steps: Vec<StepResult>,
        total_duration_ms: u64,
        error: String,
    },
}

/// Result of executing a single workflow step.
#[derive(Debug, Clone)]
pub struct StepResult {
    pub step_id: String,
    pub skill_name: String,
    pub status: StepStatus,
    pub output: Option<JsonValue>,
    pub error: Option<String>,
    pub duration_ms: u64,
}

/// Status of a step execution.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepStatus {
    Success,
    Failed,
    Skipped,
}

impl std::fmt::Display for StepStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Success => write!(f, "success"),
            Self::Failed => write!(f, "failed"),
            Self::Skipped => write!(f, "skipped"),
        }
    }
}

// =============================================================================
// WORKFLOW ENGINE
// =============================================================================

/// Database-backed workflow engine with validation, execution, and skill chaining.
pub struct WorkflowEngine {
    pool: PgPool,
    event_stream: Option<Arc<EventStream>>,
}

impl WorkflowEngine {
    /// Create a new `WorkflowEngine`.
    pub fn new(pool: PgPool, event_stream: Option<Arc<EventStream>>) -> Self {
        Self { pool, event_stream }
    }

    // =========================================================================
    // CRUD OPERATIONS (Step 2)
    // =========================================================================

    /// Create a new workflow definition.
    pub async fn create_workflow(
        &self,
        name: String,
        description: Option<String>,
        steps: Vec<WorkflowStepDef>,
        created_by: Uuid,
    ) -> Result<WorkflowDefinition> {
        // Validate before persisting
        self.validate_workflow(&steps).await?;

        let skill_chain = serde_json::to_value(&steps)
            .map_err(|e| Error::Worker(format!("Failed to serialize steps: {e}")))?;

        let row = sqlx::query(
            r"
            INSERT INTO workflows (name, description, created_by, skill_chain, enabled)
            VALUES ($1, $2, $3, $4, true)
            RETURNING workflow_id, name, description, created_by, skill_chain,
                      enabled, created_at, updated_at
            ",
        )
        .bind(&name)
        .bind(&description)
        .bind(created_by)
        .bind(&skill_chain)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Error::Worker(format!("Failed to create workflow: {e}")))?;

        let workflow = self.row_to_definition(&row)?;

        self.emit_event(
            EventType::WorkflowCreated,
            json!({
                "workflow_id": workflow.workflow_id,
                "name": &workflow.name,
                "steps_count": workflow.steps.len(),
            }),
        );

        tracing::info!(
            workflow_id = %workflow.workflow_id,
            name = %workflow.name,
            steps = workflow.steps.len(),
            "Workflow created"
        );

        Ok(workflow)
    }

    /// Get a workflow by ID.
    pub async fn get_workflow(&self, workflow_id: Uuid) -> Result<Option<WorkflowDefinition>> {
        let row = sqlx::query(
            r"
            SELECT workflow_id, name, description, created_by, skill_chain,
                   enabled, created_at, updated_at
            FROM workflows
            WHERE workflow_id = $1
            ",
        )
        .bind(workflow_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|e| Error::Worker(format!("Failed to get workflow: {e}")))?;

        match row {
            Some(r) => Ok(Some(self.row_to_definition(&r)?)),
            None => Ok(None),
        }
    }

    /// List workflows with optional enabled-only filter.
    pub async fn list_workflows(&self, enabled_only: bool) -> Result<Vec<WorkflowDefinition>> {
        let rows = if enabled_only {
            sqlx::query(
                r"
                SELECT workflow_id, name, description, created_by, skill_chain,
                       enabled, created_at, updated_at
                FROM workflows
                WHERE enabled = true
                ORDER BY created_at DESC
                ",
            )
            .fetch_all(&self.pool)
            .await
        } else {
            sqlx::query(
                r"
                SELECT workflow_id, name, description, created_by, skill_chain,
                       enabled, created_at, updated_at
                FROM workflows
                ORDER BY created_at DESC
                ",
            )
            .fetch_all(&self.pool)
            .await
        }
        .map_err(|e| Error::Worker(format!("Failed to list workflows: {e}")))?;

        let mut workflows = Vec::with_capacity(rows.len());
        for row in &rows {
            workflows.push(self.row_to_definition(row)?);
        }
        Ok(workflows)
    }

    /// Update a workflow definition.
    pub async fn update_workflow(
        &self,
        workflow_id: Uuid,
        name: Option<String>,
        description: Option<String>,
        steps: Option<Vec<WorkflowStepDef>>,
    ) -> Result<WorkflowDefinition> {
        // If steps are being updated, validate them first
        if let Some(ref new_steps) = steps {
            self.validate_workflow(new_steps).await?;
        }

        let existing = self
            .get_workflow(workflow_id)
            .await?
            .ok_or_else(|| Error::Config(format!("Workflow {workflow_id} not found")))?;

        let final_name = name.unwrap_or(existing.name);
        let final_description = description.or(existing.description);
        let final_steps = steps.unwrap_or(existing.steps);

        let skill_chain = serde_json::to_value(&final_steps)
            .map_err(|e| Error::Worker(format!("Failed to serialize steps: {e}")))?;

        let row = sqlx::query(
            r"
            UPDATE workflows
            SET name = $1, description = $2, skill_chain = $3, updated_at = NOW()
            WHERE workflow_id = $4
            RETURNING workflow_id, name, description, created_by, skill_chain,
                      enabled, created_at, updated_at
            ",
        )
        .bind(&final_name)
        .bind(&final_description)
        .bind(&skill_chain)
        .bind(workflow_id)
        .fetch_one(&self.pool)
        .await
        .map_err(|e| Error::Worker(format!("Failed to update workflow: {e}")))?;

        let workflow = self.row_to_definition(&row)?;

        self.emit_event(
            EventType::WorkflowUpdated,
            json!({
                "workflow_id": workflow.workflow_id,
                "name": &workflow.name,
            }),
        );

        tracing::info!(workflow_id = %workflow.workflow_id, "Workflow updated");

        Ok(workflow)
    }

    /// Soft-delete a workflow by setting `enabled = false`.
    pub async fn delete_workflow(&self, workflow_id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            "UPDATE workflows SET enabled = false, updated_at = NOW() WHERE workflow_id = $1 AND enabled = true",
        )
        .bind(workflow_id)
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Worker(format!("Failed to delete workflow: {e}")))?;

        if result.rows_affected() == 0 {
            return Ok(false);
        }

        self.emit_event(
            EventType::WorkflowDeleted,
            json!({ "workflow_id": workflow_id }),
        );

        tracing::info!(workflow_id = %workflow_id, "Workflow soft-deleted");

        Ok(true)
    }

    // =========================================================================
    // VALIDATION (Step 3)
    // =========================================================================

    /// Validate a workflow definition: unique step IDs, no circular deps, skills exist.
    pub async fn validate_workflow(&self, steps: &[WorkflowStepDef]) -> Result<()> {
        if steps.is_empty() {
            return Err(Error::Config("Workflow must have at least one step".into()));
        }

        // Check unique step IDs
        let mut seen_ids = HashSet::new();
        for step in steps {
            if step.step_id.trim().is_empty() {
                return Err(Error::Config("Step ID cannot be empty".into()));
            }
            if !seen_ids.insert(&step.step_id) {
                return Err(Error::Config(format!(
                    "Duplicate step ID: {}",
                    step.step_id
                )));
            }
        }

        // Check all depends_on references exist
        for step in steps {
            for dep in &step.depends_on {
                if !seen_ids.contains(dep) {
                    return Err(Error::Config(format!(
                        "Step '{}' depends on unknown step '{}'",
                        step.step_id, dep
                    )));
                }
            }
        }

        // Check for circular dependencies
        self.check_circular_dependencies(steps)?;

        // Verify all referenced skills exist in the database
        self.verify_skills_exist(steps).await?;

        Ok(())
    }

    /// Build an adjacency list from step dependencies.
    #[allow(dead_code)]
    fn build_dependency_graph(steps: &[WorkflowStepDef]) -> HashMap<String, Vec<String>> {
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();
        for step in steps {
            graph.entry(step.step_id.clone()).or_default();
            for dep in &step.depends_on {
                graph
                    .entry(dep.clone())
                    .or_default()
                    .push(step.step_id.clone());
            }
        }
        graph
    }

    /// Check for circular dependencies using Kahn's algorithm (topological sort).
    fn check_circular_dependencies(&self, steps: &[WorkflowStepDef]) -> Result<()> {
        #![allow(clippy::unused_self)]
        // We just run topological_sort and check for errors
        let _ = Self::topological_sort(steps)?;
        Ok(())
    }

    /// Return execution order respecting dependencies via topological sort.
    /// Returns an error if a circular dependency is detected.
    pub fn topological_sort(steps: &[WorkflowStepDef]) -> Result<Vec<String>> {
        let mut in_degree: HashMap<String, usize> = HashMap::new();
        let mut adjacency: HashMap<String, Vec<String>> = HashMap::new();

        for step in steps {
            in_degree.entry(step.step_id.clone()).or_insert(0);
            adjacency.entry(step.step_id.clone()).or_default();
        }

        for step in steps {
            for dep in &step.depends_on {
                adjacency
                    .entry(dep.clone())
                    .or_default()
                    .push(step.step_id.clone());
                *in_degree.entry(step.step_id.clone()).or_insert(0) += 1;
            }
        }

        let mut queue: VecDeque<String> = VecDeque::new();
        for (id, &deg) in &in_degree {
            if deg == 0 {
                queue.push_back(id.clone());
            }
        }

        let mut order = Vec::new();
        while let Some(node) = queue.pop_front() {
            order.push(node.clone());
            if let Some(neighbors) = adjacency.get(&node) {
                for neighbor in neighbors {
                    if let Some(deg) = in_degree.get_mut(neighbor) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push_back(neighbor.clone());
                        }
                    }
                }
            }
        }

        if order.len() != steps.len() {
            return Err(Error::Config(
                "Circular dependency detected in workflow steps".into(),
            ));
        }

        Ok(order)
    }

    /// Verify all skill_name references in steps exist in the skills table.
    async fn verify_skills_exist(&self, steps: &[WorkflowStepDef]) -> Result<()> {
        let skill_names: Vec<String> = steps.iter().map(|s| s.skill_name.clone()).collect();
        let unique_names: HashSet<&str> = skill_names.iter().map(|s| s.as_str()).collect();

        for skill_name in &unique_names {
            let exists: bool = sqlx::query_scalar(
                "SELECT EXISTS(SELECT 1 FROM skills WHERE name = $1 AND enabled = true)",
            )
            .bind(skill_name)
            .fetch_one(&self.pool)
            .await
            .unwrap_or(false);

            if !exists {
                return Err(Error::Config(format!(
                    "Skill '{}' not found or not enabled",
                    skill_name
                )));
            }
        }

        Ok(())
    }

    // =========================================================================
    // EXECUTION ENGINE (Steps 4, 10, 11)
    // =========================================================================

    /// Execute a workflow by ID with the given input.
    pub async fn execute_workflow(
        &self,
        workflow_id: Uuid,
        input: JsonValue,
        correlation_id: Option<Uuid>,
    ) -> Result<WorkflowExecutionResponse> {
        let workflow = self
            .get_workflow(workflow_id)
            .await?
            .ok_or_else(|| Error::Config(format!("Workflow {workflow_id} not found")))?;

        if !workflow.enabled {
            return Err(Error::Config("Workflow is disabled".into()));
        }

        self.execute_workflow_definition(&workflow, input, correlation_id)
            .await
    }

    /// Execute a workflow from a definition (supports both persisted and ephemeral workflows).
    pub async fn execute_workflow_definition(
        &self,
        workflow: &WorkflowDefinition,
        input: JsonValue,
        correlation_id: Option<Uuid>,
    ) -> Result<WorkflowExecutionResponse> {
        let correlation = correlation_id.unwrap_or_else(Uuid::now_v7);
        let start = Instant::now();

        self.emit_event(
            EventType::WorkflowExecutionStarted,
            json!({
                "workflow_id": workflow.workflow_id,
                "workflow_name": &workflow.name,
                "correlation_id": correlation,
            }),
        );

        tracing::info!(
            workflow_id = %workflow.workflow_id,
            workflow_name = %workflow.name,
            correlation_id = %correlation,
            "Workflow execution started"
        );

        // Determine execution order
        let execution_order = Self::topological_sort(&workflow.steps)?;

        // Build step lookup
        let step_map: HashMap<&str, &WorkflowStepDef> = workflow
            .steps
            .iter()
            .map(|s| (s.step_id.as_str(), s))
            .collect();

        // Initialize execution context
        let mut context = WorkflowExecutionContext {
            input: input.clone(),
            step_outputs: HashMap::new(),
            variables: HashMap::new(),
        };

        let mut step_results: Vec<StepResult> = Vec::new();
        let mut has_failure = false;

        for step_id in &execution_order {
            let step = step_map
                .get(step_id.as_str())
                .ok_or_else(|| Error::Worker(format!("Step '{step_id}' not in map")))?;

            // Check if any dependency failed (and this step doesn't have continue_on_error)
            let dep_failed = step.depends_on.iter().any(|dep_id| {
                step_results
                    .iter()
                    .any(|r| r.step_id == *dep_id && r.status == StepStatus::Failed)
            });

            if dep_failed && !step.continue_on_error {
                step_results.push(StepResult {
                    step_id: step.step_id.clone(),
                    skill_name: step.skill_name.clone(),
                    status: StepStatus::Skipped,
                    output: None,
                    error: Some("Skipped due to dependency failure".into()),
                    duration_ms: 0,
                });
                continue;
            }

            // Evaluate condition if present
            if let Some(ref condition) = step.condition {
                if !self.evaluate_condition(condition, &context) {
                    step_results.push(StepResult {
                        step_id: step.step_id.clone(),
                        skill_name: step.skill_name.clone(),
                        status: StepStatus::Skipped,
                        output: None,
                        error: Some("Condition evaluated to false".into()),
                        duration_ms: 0,
                    });
                    continue;
                }
            }

            // Execute the step (with retry logic)
            let result = self
                .execute_step_with_retry(step, &mut context, correlation)
                .await;

            if result.status == StepStatus::Failed {
                has_failure = true;
            }

            self.emit_event(
                EventType::WorkflowStepCompleted,
                json!({
                    "workflow_id": workflow.workflow_id,
                    "step_id": &result.step_id,
                    "skill_name": &result.skill_name,
                    "status": result.status.to_string(),
                    "duration_ms": result.duration_ms,
                    "correlation_id": correlation,
                }),
            );

            step_results.push(result);
        }

        let total_duration_ms = start.elapsed().as_millis() as u64;
        let successful_steps = step_results
            .iter()
            .filter(|r| r.status == StepStatus::Success)
            .count();
        let failed_steps = step_results
            .iter()
            .filter(|r| r.status == StepStatus::Failed)
            .count();

        let (status, execution_summary) = if !has_failure {
            (
                "success".to_string(),
                format!("All {} steps completed successfully", successful_steps),
            )
        } else if successful_steps > 0 {
            (
                "partial_success".to_string(),
                format!(
                    "{} of {} steps succeeded, {} failed",
                    successful_steps,
                    step_results.len(),
                    failed_steps
                ),
            )
        } else {
            (
                "failed".to_string(),
                format!("All {} steps failed", failed_steps),
            )
        };

        let event_type = if has_failure {
            EventType::WorkflowExecutionFailed
        } else {
            EventType::WorkflowExecutionCompleted
        };

        self.emit_event(
            event_type,
            json!({
                "workflow_id": workflow.workflow_id,
                "workflow_name": &workflow.name,
                "status": &status,
                "total_duration_ms": total_duration_ms,
                "successful_steps": successful_steps,
                "failed_steps": failed_steps,
                "correlation_id": correlation,
            }),
        );

        // Store execution history in task_runs (Step 10)
        self.store_execution_history(
            workflow,
            &step_results,
            total_duration_ms,
            &status,
            correlation,
        )
        .await;

        tracing::info!(
            workflow_id = %workflow.workflow_id,
            status = %status,
            total_duration_ms = total_duration_ms,
            successful_steps = successful_steps,
            failed_steps = failed_steps,
            "Workflow execution completed"
        );

        let detail_steps: Vec<StepResultDetail> = step_results
            .iter()
            .map(|r| StepResultDetail {
                step_id: r.step_id.clone(),
                skill_name: r.skill_name.clone(),
                status: r.status.to_string(),
                output: r.output.clone(),
                error: r.error.clone(),
                duration_ms: r.duration_ms,
            })
            .collect();

        Ok(WorkflowExecutionResponse {
            workflow_id: workflow.workflow_id,
            workflow_name: workflow.name.clone(),
            status,
            steps: detail_steps,
            total_duration_ms,
            successful_steps,
            failed_steps,
            execution_summary,
            correlation_id: Some(correlation),
        })
    }

    /// Execute a single step with retry logic (Step 11).
    async fn execute_step_with_retry(
        &self,
        step: &WorkflowStepDef,
        context: &mut WorkflowExecutionContext,
        correlation_id: Uuid,
    ) -> StepResult {
        let max_attempts = step
            .retry_policy
            .as_ref()
            .map_or(1, |p| p.max_attempts.max(1));
        let delay_secs = step.retry_policy.as_ref().map_or(5, |p| p.delay_secs);

        let mut last_result = None;

        for attempt in 1..=max_attempts {
            let result = self.execute_step(step, context, correlation_id).await;

            if result.status == StepStatus::Success {
                // Store output in context for downstream steps
                if let Some(ref output) = result.output {
                    context
                        .step_outputs
                        .insert(step.step_id.clone(), output.clone());
                }
                return result;
            }

            tracing::warn!(
                step_id = %step.step_id,
                attempt = attempt,
                max_attempts = max_attempts,
                error = ?result.error,
                "Workflow step failed"
            );

            last_result = Some(result);

            if attempt < max_attempts {
                tokio::time::sleep(std::time::Duration::from_secs(delay_secs)).await;
            }
        }

        last_result.unwrap_or_else(|| StepResult {
            step_id: step.step_id.clone(),
            skill_name: step.skill_name.clone(),
            status: StepStatus::Failed,
            output: None,
            error: Some("Step execution produced no result".into()),
            duration_ms: 0,
        })
    }

    /// Execute a single workflow step by creating a task and polling for completion.
    async fn execute_step(
        &self,
        step: &WorkflowStepDef,
        context: &WorkflowExecutionContext,
        correlation_id: Uuid,
    ) -> StepResult {
        let step_start = Instant::now();

        // Resolve input parameters from context
        let resolved_input = step.input_mapping.as_ref().map_or_else(
            || json!({}),
            |mapping| Self::resolve_input(mapping, context),
        );

        // Look up the skill_id for this skill_name
        let skill_id: Option<Uuid> =
            sqlx::query_scalar("SELECT skill_id FROM skills WHERE name = $1 AND enabled = true")
                .bind(&step.skill_name)
                .fetch_optional(&self.pool)
                .await
                .ok()
                .flatten();

        let skill_id = match skill_id {
            Some(id) => id,
            None => {
                return StepResult {
                    step_id: step.step_id.clone(),
                    skill_name: step.skill_name.clone(),
                    status: StepStatus::Failed,
                    output: None,
                    error: Some(format!("Skill '{}' not found or disabled", step.skill_name)),
                    duration_ms: step_start.elapsed().as_millis() as u64,
                };
            }
        };

        // Create a task for this step
        let task_id = Uuid::now_v7();
        let title = format!("Workflow step: {} ({})", step.step_id, step.skill_name);
        let description = serde_json::to_string(&resolved_input).unwrap_or_default();

        let insert_result = sqlx::query(
            r"
            INSERT INTO tasks (task_id, title, description, skill_id, state, priority, correlation_id)
            VALUES ($1, $2, $3, $4, 'pending', 5, $5)
            ",
        )
        .bind(task_id)
        .bind(&title)
        .bind(&description)
        .bind(skill_id)
        .bind(correlation_id)
        .execute(&self.pool)
        .await;

        if let Err(e) = insert_result {
            return StepResult {
                step_id: step.step_id.clone(),
                skill_name: step.skill_name.clone(),
                status: StepStatus::Failed,
                output: None,
                error: Some(format!("Failed to create task: {e}")),
                duration_ms: step_start.elapsed().as_millis() as u64,
            };
        }

        // Task is now pending — the Scheduler will dequeue it, set it to
        // 'running', create the task_runs record, and invoke the skill.
        // We poll the task state and the latest task_runs row for completion.
        let timeout = std::time::Duration::from_secs(300); // 5 minute timeout
        let poll_interval = std::time::Duration::from_millis(500);
        let poll_start = Instant::now();

        loop {
            if poll_start.elapsed() > timeout {
                // Mark the task as failed so the scheduler stops retrying
                let _ = sqlx::query(
                    "UPDATE tasks SET state = 'failed', updated_at = NOW() WHERE task_id = $1",
                )
                .bind(task_id)
                .execute(&self.pool)
                .await;

                return StepResult {
                    step_id: step.step_id.clone(),
                    skill_name: step.skill_name.clone(),
                    status: StepStatus::Failed,
                    output: None,
                    error: Some("Step execution timed out after 300s".into()),
                    duration_ms: step_start.elapsed().as_millis() as u64,
                };
            }

            // Check the latest task_runs row for this task (by max attempt)
            let run_state: Option<(String, Option<JsonValue>, Option<String>)> = sqlx::query_as(
                "SELECT state, result, error FROM task_runs WHERE task_id = $1 ORDER BY attempt DESC LIMIT 1",
            )
            .bind(task_id)
            .fetch_optional(&self.pool)
            .await
            .ok()
            .flatten();

            if let Some((state, result, error)) = run_state {
                match state.as_str() {
                    "success" => {
                        return StepResult {
                            step_id: step.step_id.clone(),
                            skill_name: step.skill_name.clone(),
                            status: StepStatus::Success,
                            output: result,
                            error: None,
                            duration_ms: step_start.elapsed().as_millis() as u64,
                        };
                    }
                    "failed" | "timeout" | "canceled" => {
                        // Also check the task-level state — the scheduler may
                        // have exhausted retries and marked the task as failed.
                        let task_state: Option<String> =
                            sqlx::query_scalar("SELECT state FROM tasks WHERE task_id = $1")
                                .bind(task_id)
                                .fetch_optional(&self.pool)
                                .await
                                .ok()
                                .flatten();

                        let is_terminal = matches!(
                            task_state.as_deref(),
                            Some("completed") | Some("failed") | Some("canceled")
                        );

                        if is_terminal {
                            return StepResult {
                                step_id: step.step_id.clone(),
                                skill_name: step.skill_name.clone(),
                                status: StepStatus::Failed,
                                output: result,
                                error: error
                                    .or_else(|| Some(format!("Step ended with state: {state}"))),
                                duration_ms: step_start.elapsed().as_millis() as u64,
                            };
                        }
                        // Task may still be retrying — continue polling
                    }
                    _ => {
                        // Still running or queued, continue polling
                    }
                }
            } else {
                // No task_runs row yet — the scheduler hasn't picked it up.
                // Also check if the task itself reached a terminal state
                // (e.g. cancelled externally).
                let task_state: Option<String> =
                    sqlx::query_scalar("SELECT state FROM tasks WHERE task_id = $1")
                        .bind(task_id)
                        .fetch_optional(&self.pool)
                        .await
                        .ok()
                        .flatten();

                if matches!(task_state.as_deref(), Some("canceled")) {
                    return StepResult {
                        step_id: step.step_id.clone(),
                        skill_name: step.skill_name.clone(),
                        status: StepStatus::Failed,
                        output: None,
                        error: Some("Task was cancelled before execution".into()),
                        duration_ms: step_start.elapsed().as_millis() as u64,
                    };
                }
            }

            tokio::time::sleep(poll_interval).await;
        }
    }

    /// Resolve input mapping expressions against the execution context.
    ///
    /// Supports JSONPath-like expressions:
    /// - `$.input.param_name` - Access workflow input parameter
    /// - `$.steps.step_id.output.field` - Access output from specific step
    /// - `$.context.variable_name` - Access shared context variable
    /// - Literal values are passed through as-is
    pub fn resolve_input(mapping: &JsonValue, context: &WorkflowExecutionContext) -> JsonValue {
        match mapping {
            JsonValue::Object(map) => {
                let mut resolved = serde_json::Map::new();
                for (key, value) in map {
                    resolved.insert(key.clone(), Self::resolve_value(value, context));
                }
                JsonValue::Object(resolved)
            }
            other => Self::resolve_value(other, context),
        }
    }

    /// Resolve a single value expression.
    fn resolve_value(value: &JsonValue, context: &WorkflowExecutionContext) -> JsonValue {
        if let Some(expr) = value.as_str() {
            if let Some(path) = expr.strip_prefix("$.") {
                return Self::resolve_path(path, context);
            }
        }
        // Literal value - return as-is
        value.clone()
    }

    /// Resolve a dotted path against the context.
    fn resolve_path(path: &str, context: &WorkflowExecutionContext) -> JsonValue {
        let parts: Vec<&str> = path.splitn(2, '.').collect();
        if parts.is_empty() {
            return JsonValue::Null;
        }

        match parts.first().copied() {
            Some("input") => {
                if parts.len() > 1 {
                    Self::navigate_json(&context.input, parts[1])
                } else {
                    context.input.clone()
                }
            }
            Some("steps") => {
                if parts.len() > 1 {
                    // parts[1] is "step_id.output.field..."
                    let rest: Vec<&str> = parts[1].splitn(2, '.').collect();
                    let step_id = rest[0];
                    context
                        .step_outputs
                        .get(step_id)
                        .map_or(JsonValue::Null, |step_output| {
                            if rest.len() > 1 {
                                Self::navigate_json(step_output, rest[1])
                            } else {
                                step_output.clone()
                            }
                        })
                } else {
                    JsonValue::Null
                }
            }
            Some("context") => {
                if parts.len() > 1 {
                    context
                        .variables
                        .get(parts[1])
                        .cloned()
                        .unwrap_or(JsonValue::Null)
                } else {
                    JsonValue::Null
                }
            }
            _ => JsonValue::Null,
        }
    }

    /// Navigate a JSON value by a dotted path.
    fn navigate_json(value: &JsonValue, path: &str) -> JsonValue {
        let mut current = value;
        for segment in path.split('.') {
            match current {
                JsonValue::Object(map) => {
                    current = map.get(segment).unwrap_or(&JsonValue::Null);
                }
                JsonValue::Array(arr) => {
                    if let Ok(idx) = segment.parse::<usize>() {
                        current = arr.get(idx).unwrap_or(&JsonValue::Null);
                    } else {
                        return JsonValue::Null;
                    }
                }
                _ => return JsonValue::Null,
            }
        }
        current.clone()
    }

    /// Evaluate a condition expression against the context.
    /// Simple implementation: checks if the condition resolves to a truthy value.
    #[allow(clippy::unused_self)]
    fn evaluate_condition(
        &self,
        condition: &JsonValue,
        context: &WorkflowExecutionContext,
    ) -> bool {
        // If condition is a string starting with "$.", resolve it
        if let Some(expr) = condition.as_str() {
            if let Some(path) = expr.strip_prefix("$.") {
                let resolved = Self::resolve_path(path, context);
                return match resolved {
                    JsonValue::Bool(b) => b,
                    JsonValue::Null => false,
                    JsonValue::Number(n) => n.as_f64().is_some_and(|v| v != 0.0),
                    JsonValue::String(s) => !s.is_empty(),
                    _ => true,
                };
            }
        }

        // If condition is a boolean literal
        if let Some(b) = condition.as_bool() {
            return b;
        }

        // If condition is an object with "exists" key, check if the path resolves to non-null
        if let Some(obj) = condition.as_object() {
            if let Some(exists_val) = obj.get("exists") {
                if let Some(path_str) = exists_val.as_str() {
                    if let Some(path) = path_str.strip_prefix("$.") {
                        let resolved = Self::resolve_path(path, context);
                        return !resolved.is_null();
                    }
                }
            }
        }

        // Default: condition is truthy
        true
    }

    // =========================================================================
    // AUTOMATIC SKILL CHAINING (Step 5)
    // =========================================================================

    /// Analyze a task description and identify matching skills by keyword matching.
    pub async fn analyze_task_for_skills(&self, task_description: &str) -> Result<Vec<String>> {
        let desc_lower = task_description.to_lowercase();

        // Fetch all enabled skills
        let rows = sqlx::query("SELECT name, description FROM skills WHERE enabled = true")
            .fetch_all(&self.pool)
            .await
            .map_err(|e| Error::Worker(format!("Failed to query skills: {e}")))?;

        let mut matches: Vec<(String, usize)> = Vec::new();

        for row in &rows {
            let skill_name: String = row.get("name");
            let skill_desc: Option<String> = row.get("description");
            let mut score = 0usize;

            // Check if skill name appears in description
            if desc_lower.contains(&skill_name.to_lowercase()) {
                score += 10;
            }

            // Check if any words from skill description appear in task description
            if let Some(ref desc) = skill_desc {
                for word in desc.split_whitespace() {
                    let w = word.to_lowercase();
                    if w.len() > 3 && desc_lower.contains(&w) {
                        score += 1;
                    }
                }
            }

            // Check for common skill-related keywords
            let name_lower = skill_name.to_lowercase();
            let keywords = [
                "analyze", "test", "review", "build", "deploy", "report", "lint", "format", "check",
            ];
            for kw in &keywords {
                if name_lower.contains(kw) && desc_lower.contains(kw) {
                    score += 5;
                }
            }

            if score > 0 {
                matches.push((skill_name, score));
            }
        }

        // Sort by score descending
        matches.sort_by(|a, b| b.1.cmp(&a.1));

        Ok(matches.into_iter().map(|(name, _)| name).collect())
    }

    /// Create an ephemeral workflow from a list of skill names with linear dependencies.
    pub fn create_workflow_from_skills(
        skill_names: Vec<String>,
        task_description: String,
    ) -> WorkflowDefinition {
        let mut steps = Vec::with_capacity(skill_names.len());
        let mut prev_step_id: Option<String> = None;

        for (i, skill_name) in skill_names.iter().enumerate() {
            let step_id = format!("step_{}", i + 1);
            let mut input_mapping = serde_json::Map::new();

            if let Some(ref prev) = prev_step_id {
                // Chain output from previous step
                input_mapping.insert(
                    "previous_output".to_string(),
                    json!(format!("$.steps.{prev}.output")),
                );
            } else {
                // First step gets the workflow input
                input_mapping.insert("input".to_string(), json!("$.input"));
            }

            let depends_on = prev_step_id.iter().cloned().collect();

            steps.push(WorkflowStepDef {
                step_id: step_id.clone(),
                skill_name: skill_name.clone(),
                input_mapping: Some(JsonValue::Object(input_mapping)),
                depends_on,
                condition: None,
                retry_policy: None,
                continue_on_error: false,
            });

            prev_step_id = Some(step_id);
        }

        WorkflowDefinition {
            workflow_id: Uuid::now_v7(),
            name: task_description,
            description: Some("Auto-generated workflow from skill chaining".into()),
            created_by: None,
            steps,
            enabled: true,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Analyze a task, create an ephemeral workflow, and execute it.
    pub async fn execute_task_with_chaining(
        &self,
        task_id: Uuid,
    ) -> Result<WorkflowExecutionResponse> {
        // Fetch task description
        let row = sqlx::query("SELECT title, description FROM tasks WHERE task_id = $1")
            .bind(task_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| Error::Worker(format!("Failed to fetch task: {e}")))?
            .ok_or_else(|| Error::Config(format!("Task {task_id} not found")))?;

        let title: String = row.get("title");
        let description: Option<String> = row.get("description");
        let full_desc = format!("{} {}", title, description.unwrap_or_default());

        // Analyze for skills
        let skill_names = self.analyze_task_for_skills(&full_desc).await?;

        if skill_names.is_empty() {
            return Err(Error::Config(
                "No matching skills found for task description".into(),
            ));
        }

        tracing::info!(
            task_id = %task_id,
            skills = ?skill_names,
            "Auto-chaining skills for task"
        );

        // Create ephemeral workflow
        let workflow = Self::create_workflow_from_skills(skill_names, title);

        // Execute with task_id as correlation
        self.execute_workflow_definition(
            &workflow,
            json!({"task_id": task_id.to_string()}),
            Some(task_id),
        )
        .await
    }

    // =========================================================================
    // SCHEDULER INTEGRATION (Step 6)
    // =========================================================================

    /// Create a workflow dispatch task in the tasks table.
    pub async fn dispatch_workflow_task(
        &self,
        workflow_id: Uuid,
        input: JsonValue,
        priority: i32,
    ) -> Result<Uuid> {
        let task_id = Uuid::now_v7();
        let workflow = self
            .get_workflow(workflow_id)
            .await?
            .ok_or_else(|| Error::Config(format!("Workflow {workflow_id} not found")))?;

        let description = json!({
            "_workflow_dispatch": true,
            "workflow_id": workflow_id,
            "workflow_name": &workflow.name,
            "input": &input,
        })
        .to_string();

        sqlx::query(
            r"
            INSERT INTO tasks (task_id, title, description, state, priority, correlation_id)
            VALUES ($1, $2, $3, 'pending', $4, $5)
            ",
        )
        .bind(task_id)
        .bind(format!("Workflow: {}", workflow.name))
        .bind(&description)
        .bind(priority)
        .bind(task_id) // self-referencing correlation
        .execute(&self.pool)
        .await
        .map_err(|e| Error::Worker(format!("Failed to dispatch workflow task: {e}")))?;

        tracing::info!(
            task_id = %task_id,
            workflow_id = %workflow_id,
            "Workflow task dispatched"
        );

        Ok(task_id)
    }

    /// Check if a task is a workflow dispatch task and execute it if so.
    /// Returns `Some(response)` if it was a workflow task, `None` otherwise.
    pub async fn try_execute_workflow_task(
        &self,
        task_id: Uuid,
    ) -> Result<Option<WorkflowExecutionResponse>> {
        let row = sqlx::query("SELECT description FROM tasks WHERE task_id = $1")
            .bind(task_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(|e| Error::Worker(format!("Failed to fetch task: {e}")))?;

        let description = match row {
            Some(r) => {
                let desc: Option<String> = r.get("description");
                desc.unwrap_or_default()
            }
            None => return Ok(None),
        };

        // Try to parse as workflow dispatch
        let parsed: std::result::Result<JsonValue, _> = serde_json::from_str(&description);
        let dispatch = match parsed {
            Ok(ref v) if v.get("_workflow_dispatch").and_then(|d| d.as_bool()) == Some(true) => v,
            _ => return Ok(None),
        };

        let workflow_id = dispatch
            .get("workflow_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .ok_or_else(|| Error::Worker("Missing workflow_id in dispatch".into()))?;

        let input = dispatch.get("input").cloned().unwrap_or_else(|| json!({}));

        let result = self
            .execute_workflow(workflow_id, input, Some(task_id))
            .await?;

        // Update the dispatch task state based on result
        let final_state = if result.status == "success" {
            "completed"
        } else {
            "failed"
        };
        let _ = sqlx::query("UPDATE tasks SET state = $1, updated_at = NOW() WHERE task_id = $2")
            .bind(final_state)
            .bind(task_id)
            .execute(&self.pool)
            .await;

        Ok(Some(result))
    }

    // =========================================================================
    // EXECUTION HISTORY (Step 10)
    // =========================================================================

    /// Store workflow execution results in the task_runs table.
    async fn store_execution_history(
        &self,
        workflow: &WorkflowDefinition,
        step_results: &[StepResult],
        total_duration_ms: u64,
        status: &str,
        correlation_id: Uuid,
    ) {
        let successful_steps = step_results
            .iter()
            .filter(|r| r.status == StepStatus::Success)
            .count();
        let failed_steps = step_results
            .iter()
            .filter(|r| r.status == StepStatus::Failed)
            .count();

        let steps_json: Vec<JsonValue> = step_results
            .iter()
            .map(|r| {
                json!({
                    "step_id": r.step_id,
                    "skill_name": r.skill_name,
                    "status": r.status.to_string(),
                    "duration_ms": r.duration_ms,
                    "output": r.output,
                    "error": r.error,
                })
            })
            .collect();

        let result_json = json!({
            "workflow_id": workflow.workflow_id,
            "workflow_name": &workflow.name,
            "steps": steps_json,
            "total_duration_ms": total_duration_ms,
            "successful_steps": successful_steps,
            "failed_steps": failed_steps,
            "execution_summary": format!("{} steps executed", step_results.len()),
        });

        // Create a parent task for the workflow execution if one doesn't exist
        let task_id = Uuid::now_v7();
        let _ = sqlx::query(
            r"
            INSERT INTO tasks (task_id, title, description, state, priority, correlation_id)
            VALUES ($1, $2, $3, $4, 0, $5)
            ",
        )
        .bind(task_id)
        .bind(format!("Workflow execution: {}", workflow.name))
        .bind(format!(
            "Workflow {} execution record",
            workflow.workflow_id
        ))
        .bind(if status == "success" {
            "completed"
        } else {
            "failed"
        })
        .bind(correlation_id)
        .execute(&self.pool)
        .await;

        let run_state = if status == "success" {
            "success"
        } else {
            "failed"
        };

        let _ = sqlx::query(
            r"
            INSERT INTO task_runs (run_id, task_id, attempt, state, started_at, ended_at, result, correlation_id)
            VALUES ($1, $2, 1, $3, NOW() - INTERVAL '1 millisecond' * $4, NOW(), $5, $6)
            ",
        )
        .bind(Uuid::now_v7())
        .bind(task_id)
        .bind(run_state)
        .bind(total_duration_ms as f64)
        .bind(&result_json)
        .bind(correlation_id)
        .execute(&self.pool)
        .await;
    }

    // =========================================================================
    // HELPERS
    // =========================================================================

    /// Parse a database row into a `WorkflowDefinition`.
    #[allow(clippy::unused_self)]
    fn row_to_definition(&self, row: &sqlx::postgres::PgRow) -> Result<WorkflowDefinition> {
        let skill_chain: JsonValue = row.get("skill_chain");
        let steps: Vec<WorkflowStepDef> = serde_json::from_value(skill_chain)
            .map_err(|e| Error::Worker(format!("Failed to deserialize skill_chain: {e}")))?;

        Ok(WorkflowDefinition {
            workflow_id: row.get("workflow_id"),
            name: row.get("name"),
            description: row.get("description"),
            created_by: row.get("created_by"),
            steps,
            enabled: row.get("enabled"),
            created_at: row.get("created_at"),
            updated_at: row.get("updated_at"),
        })
    }

    /// Emit an event to the event stream if available.
    fn emit_event(&self, event_type: EventType, payload: JsonValue) {
        if let Some(ref es) = self.event_stream {
            es.publish(EventEnvelope::new(EventLevel::Info, event_type, payload));
        }
    }
}
