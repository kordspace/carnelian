//! Model router for LLM completion requests
//!
//! The `ModelRouter` acts as a bridge between the Rust core and the TypeScript
//! gateway service running on port 18790. It implements:
//!
//! - **Local-first routing**: Prefers Ollama when the requested model is available locally
//! - **Capability-based provider enablement**: Checks `PolicyEngine` grants before
//!   dispatching to remote providers (OpenAI, Anthropic, Fireworks)
//! - **Budget enforcement**: Queries `usage_costs` to enforce daily/monthly spend limits
//!   stored in `model_providers.budget_limits`
//! - **Correlation ID propagation**: Every request carries a UUID that flows through
//!   the ledger, gateway, and usage records for end-to-end tracing
//! - **Usage persistence**: Writes token counts and cost estimates to `usage_costs`
//!   after each completion
//!
//! # Architecture
//!
//! ```text
//! Caller → ModelRouter → Gateway (HTTP) → Provider Adapter → LLM Backend
//!              │                                │
//!              ├→ PolicyEngine (capability)      └→ usage report
//!              ├→ usage_costs  (budget check)
//!              └→ Ledger       (audit trail)
//! ```
//!
//! # Context Integrity
//!
//! **Important**: The model router receives pre-assembled messages and does not
//! perform context assembly itself. Callers **must** log context integrity to the
//! ledger *before* invoking [`ModelRouter::complete`] or [`ModelRouter::complete_stream`].
//!
//! The expected correlation ID flow is:
//!
//! 1. **Context assembly**: `ContextWindow::log_to_ledger(&ledger, correlation_id)` logs
//!    a `"model.context.assembled"` event with full provenance (memory IDs, run IDs,
//!    message IDs, blake3 hash).
//! 2. **Model call**: `ModelRouter::complete(request, ...)` logs `"model.call.request"`
//!    and `"model.call.response"` events with the same `correlation_id`.
//! 3. **Audit trail**: All three events are linked by `correlation_id`, enabling
//!    post-hoc verification that the exact context was used for a given model response.
//!
//! ## Example
//!
//! ```ignore
//! // 1. Assemble context and log integrity
//! let assembled = ctx.assemble(&config).await?;
//! ctx.log_to_ledger(&ledger, correlation_id).await?;
//!
//! // 2. Build messages and call model router
//! let request = CompletionRequest {
//!     model: "deepseek-r1:7b".to_string(),
//!     messages,
//!     correlation_id: Some(correlation_id),
//!     ..Default::default()
//! };
//! let provenance = ctx.compute_provenance();
//! let response = model_router.complete(request, identity_id, task_id, run_id, Some(&provenance)).await?;
//!
//! // Ledger now contains (linked by correlation_id):
//! //   "model.context.assembled" → "model.call.request" → "model.call.response"
//! ```

use std::sync::Arc;
use std::time::Duration;

use bytes::Bytes;
use carnelian_common::{Error, Result};
use futures_util::stream::{Stream, StreamExt};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::{Value as JsonValue, json};
use sqlx::PgPool;
use uuid::Uuid;

use crate::EventStream;
use crate::context::ContextProvenance;
use crate::ledger::Ledger;
use crate::policy::PolicyEngine;

// =============================================================================
// REQUEST / RESPONSE TYPES
// =============================================================================

/// A single message in a completion request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Message {
    /// Role: `"system"`, `"user"`, `"assistant"`, or `"tool"`.
    pub role: String,
    /// Text content of the message.
    pub content: String,
    /// Optional display name.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// Tool call ID (for tool-result messages).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tool_call_id: Option<String>,
}

/// Completion request sent to the gateway.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionRequest {
    /// Model identifier (e.g. `"deepseek-r1:7b"`, `"gpt-4o"`).
    pub model: String,
    /// Conversation messages.
    pub messages: Vec<Message>,
    /// Sampling temperature (0–2).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Maximum tokens to generate.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<i32>,
    /// Whether to stream the response.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stream: Option<bool>,
    /// Correlation ID for end-to-end tracing.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlation_id: Option<Uuid>,
}

/// A single choice in a completion response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Choice {
    pub index: i32,
    pub message: Message,
    pub finish_reason: Option<String>,
}

/// Token usage statistics returned by the gateway.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UsageStats {
    pub prompt_tokens: i32,
    pub completion_tokens: i32,
    pub total_tokens: i32,
}

/// Non-streaming completion response from the gateway.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionResponse {
    pub id: String,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: UsageStats,
    pub provider: String,
}

/// A single delta in a streaming completion chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkDelta {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

/// A single choice in a streaming chunk.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChunkChoice {
    pub index: i32,
    pub delta: ChunkDelta,
    pub finish_reason: Option<String>,
}

/// A streaming completion chunk from the gateway (SSE).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CompletionChunk {
    pub id: String,
    pub model: String,
    pub choices: Vec<ChunkChoice>,
}

// =============================================================================
// PROVIDER CONFIGURATION (from database)
// =============================================================================

/// A row from the `model_providers` table.
#[derive(Debug, Clone)]
pub struct ModelProvider {
    pub provider_id: Uuid,
    /// `"local"` or `"remote"`.
    pub provider_type: String,
    /// Provider name (e.g. `"ollama"`, `"openai"`).
    pub name: String,
    pub enabled: bool,
    /// Provider-specific configuration JSONB.
    pub config: JsonValue,
    /// Budget limits JSONB: `{"daily_limit_usd": 10.0, "monthly_limit_usd": 100.0}`.
    pub budget_limits: JsonValue,
}

// =============================================================================
// GATEWAY HEALTH RESPONSE (subset)
// =============================================================================

#[derive(Debug, Deserialize)]
struct GatewayHealthResponse {
    #[allow(dead_code)]
    status: String,
    providers: Vec<GatewayProviderHealth>,
}

#[derive(Debug, Deserialize)]
struct GatewayProviderHealth {
    #[allow(dead_code)]
    name: String,
    available: bool,
    models: Option<Vec<String>>,
}

// =============================================================================
// MODEL ROUTER
// =============================================================================

/// Routes LLM completion requests through the TypeScript gateway with
/// capability checks, budget enforcement, and usage persistence.
pub struct ModelRouter {
    pool: PgPool,
    gateway_url: String,
    http_client: Client,
    policy_engine: Arc<PolicyEngine>,
    ledger: Arc<Ledger>,
    event_stream: Option<Arc<EventStream>>,
    /// Safe mode guard for blocking remote model calls
    safe_mode_guard: Option<Arc<crate::safe_mode::SafeModeGuard>>,
}

impl ModelRouter {
    /// Create a new `ModelRouter`.
    ///
    /// # Arguments
    ///
    /// * `pool` – Postgres connection pool for provider/usage queries.
    /// * `gateway_url` – Base URL of the TypeScript gateway (default `http://localhost:18790`).
    /// * `policy_engine` – Capability-based security engine.
    /// * `ledger` – Tamper-resistant audit ledger.
    pub fn new(
        pool: PgPool,
        gateway_url: String,
        policy_engine: Arc<PolicyEngine>,
        ledger: Arc<Ledger>,
    ) -> Self {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(120))
            .connect_timeout(Duration::from_secs(10))
            .build()
            .expect("Failed to build HTTP client");

        Self {
            pool,
            gateway_url,
            http_client,
            policy_engine,
            ledger,
            event_stream: None,
            safe_mode_guard: None,
        }
    }

    /// Attach an optional event stream for publishing routing events.
    pub fn with_event_stream(mut self, event_stream: Arc<EventStream>) -> Self {
        self.event_stream = Some(event_stream);
        self
    }

    /// Attach a safe mode guard for blocking remote model calls.
    pub fn with_safe_mode_guard(mut self, guard: Arc<crate::safe_mode::SafeModeGuard>) -> Self {
        self.safe_mode_guard = Some(guard);
        self
    }

    /// Return the gateway base URL.
    #[must_use]
    pub fn gateway_url(&self) -> &str {
        &self.gateway_url
    }

    // =========================================================================
    // PROVIDER MANAGEMENT
    // =========================================================================

    /// Load all enabled providers from the database, local providers first.
    async fn load_providers(&self) -> Result<Vec<ModelProvider>> {
        let rows: Vec<(Uuid, String, String, bool, JsonValue, JsonValue)> = sqlx::query_as(
            r"SELECT provider_id, provider_type, name, enabled, config, budget_limits
              FROM model_providers
              WHERE enabled = true
              ORDER BY provider_type ASC",
        )
        .fetch_all(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(rows
            .into_iter()
            .map(
                |(provider_id, provider_type, name, enabled, config, budget_limits)| {
                    ModelProvider {
                        provider_id,
                        provider_type,
                        name,
                        enabled,
                        config,
                        budget_limits,
                    }
                },
            )
            .collect())
    }

    /// Check whether an identity has the capability to use a given provider.
    ///
    /// Local providers (Ollama) always return `true`.
    /// Remote providers require `"model.remote"` capability.
    async fn check_provider_capability(
        &self,
        provider: &ModelProvider,
        identity_id: Uuid,
    ) -> Result<bool> {
        if provider.provider_type == "local" {
            return Ok(true);
        }

        self.policy_engine
            .check_capability(
                "identity",
                &identity_id.to_string(),
                "model.remote",
                self.event_stream.as_deref(),
            )
            .await
    }

    /// Check whether the provider is within all configured budget limits.
    ///
    /// Reads `daily_limit_usd` and `monthly_limit_usd` from `budget_limits` JSONB
    /// and compares against the sum of `cost_estimate` in `usage_costs` for the
    /// corresponding intervals. Returns `true` only if all configured limits are
    /// satisfied, or if no limits are configured.
    async fn check_budget(&self, provider: &ModelProvider) -> Result<bool> {
        let daily_limit = provider
            .budget_limits
            .get("daily_limit_usd")
            .and_then(|v| v.as_f64());
        let monthly_limit = provider
            .budget_limits
            .get("monthly_limit_usd")
            .and_then(|v| v.as_f64());

        if daily_limit.is_none() && monthly_limit.is_none() {
            return Ok(true); // No limits configured
        }

        // Check daily limit
        if let Some(limit) = daily_limit {
            let spent: Option<f64> = sqlx::query_scalar(
                r"SELECT CAST(SUM(cost_estimate) AS DOUBLE PRECISION)
                  FROM usage_costs
                  WHERE provider_id = $1 AND ts >= NOW() - INTERVAL '1 day'",
            )
            .bind(provider.provider_id)
            .fetch_one(&self.pool)
            .await
            .map_err(Error::Database)?;

            let spent = spent.unwrap_or(0.0);

            if spent >= limit {
                tracing::warn!(
                    provider = %provider.name,
                    spent = spent,
                    limit = limit,
                    "Provider daily budget exceeded"
                );
                return Ok(false);
            }
        }

        // Check monthly limit
        if let Some(limit) = monthly_limit {
            let spent: Option<f64> = sqlx::query_scalar(
                r"SELECT CAST(SUM(cost_estimate) AS DOUBLE PRECISION)
                  FROM usage_costs
                  WHERE provider_id = $1 AND ts >= NOW() - INTERVAL '30 days'",
            )
            .bind(provider.provider_id)
            .fetch_one(&self.pool)
            .await
            .map_err(Error::Database)?;

            let spent = spent.unwrap_or(0.0);

            if spent >= limit {
                tracing::warn!(
                    provider = %provider.name,
                    spent = spent,
                    limit = limit,
                    "Provider monthly budget exceeded"
                );
                return Ok(false);
            }
        }

        Ok(true)
    }

    // =========================================================================
    // MODEL ROUTING
    // =========================================================================

    /// Select the best provider for a model, enforcing capability and budget checks.
    ///
    /// Routing strategy:
    /// 1. **Local-first** – If a local provider has the model, use it.
    /// 2. **Pattern match** – Route by model name prefix to the canonical remote provider.
    /// 3. **Fallback** – Try any available remote provider with valid capability and budget.
    async fn select_provider(&self, model: &str, identity_id: Uuid) -> Result<ModelProvider> {
        let providers = self.load_providers().await?;

        if providers.is_empty() {
            return Err(Error::ModelRouting(
                "No enabled providers found in database".to_string(),
            ));
        }

        // Step 1: Local-first — check if any local provider has the model
        let local_providers: Vec<&ModelProvider> = providers
            .iter()
            .filter(|p| p.provider_type == "local")
            .collect();

        if !local_providers.is_empty()
            && matches!(self.model_available_locally(model).await, Ok(true))
        {
            // Return the first local provider
            return Ok(local_providers[0].clone());
        }

        // Step 2: Pattern-match model name to canonical remote provider
        let target_name = Self::match_provider_name(model);

        if let Some(name) = target_name {
            if let Some(provider) = providers.iter().find(|p| p.name == name) {
                if self
                    .check_provider_capability(provider, identity_id)
                    .await?
                {
                    if self.check_budget(provider).await? {
                        return Ok(provider.clone());
                    }
                    return Err(Error::BudgetExceeded(format!(
                        "Daily budget exceeded for provider '{}'",
                        provider.name
                    )));
                }
                return Err(Error::Security(format!(
                    "Identity lacks model.remote capability for provider '{}'",
                    provider.name
                )));
            }
        }

        // Step 3: Fallback — try any remote provider with valid capability and budget
        for provider in providers.iter().filter(|p| p.provider_type == "remote") {
            if self
                .check_provider_capability(provider, identity_id)
                .await?
                && self.check_budget(provider).await?
            {
                return Ok(provider.clone());
            }
        }

        Err(Error::ModelRouting(format!(
            "No suitable provider found for model '{model}'"
        )))
    }

    /// Match a model name to a canonical provider name.
    fn match_provider_name(model: &str) -> Option<&'static str> {
        let lower = model.to_lowercase();
        if lower.starts_with("claude") {
            Some("anthropic")
        } else if lower.starts_with("gpt-") || lower.starts_with("o1") || lower.starts_with("o3") {
            Some("openai")
        } else if lower.starts_with("accounts/fireworks") {
            Some("fireworks")
        } else {
            None
        }
    }

    /// Ask the gateway whether a model is available on a local provider.
    async fn model_available_locally(&self, model: &str) -> Result<bool> {
        let url = format!("{}/health", self.gateway_url);

        let resp = self
            .http_client
            .get(&url)
            .timeout(Duration::from_secs(5))
            .send()
            .await
            .map_err(|e| Error::GatewayUnavailable(format!("Health check failed: {e}")))?;

        if !resp.status().is_success() {
            return Ok(false);
        }

        let health: GatewayHealthResponse = resp
            .json()
            .await
            .map_err(|e| Error::GatewayUnavailable(format!("Invalid health response: {e}")))?;

        for provider in &health.providers {
            if !provider.available {
                continue;
            }
            if let Some(ref models) = provider.models {
                if models.iter().any(|m| m == model) {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    // =========================================================================
    // COMPLETION METHODS
    // =========================================================================

    /// Send a non-streaming completion request through the gateway.
    ///
    /// # Arguments
    ///
    /// * `request` – The completion request (model, messages, parameters).
    /// * `identity_id` – Identity performing the request (for capability checks).
    /// * `task_id` – Optional task association for usage tracking.
    /// * `run_id` – Optional run association for usage tracking.
    /// * `provenance` – Optional context provenance from `ContextWindow::compute_provenance()`.
    ///   When provided, a `model.context.assembled` ledger event is emitted **before**
    ///   the `model.call.request` event, ensuring the correlation chain is
    ///   context → call → response.
    ///
    /// # Errors
    ///
    /// Returns `ModelRouting` if no provider is available, `GatewayUnavailable`
    /// if the gateway cannot be reached, or `BudgetExceeded` if the provider's
    /// daily spend limit has been hit.
    pub async fn complete(
        &self,
        mut request: CompletionRequest,
        identity_id: Uuid,
        task_id: Option<Uuid>,
        run_id: Option<Uuid>,
        provenance: Option<&ContextProvenance>,
    ) -> Result<CompletionResponse> {
        let correlation_id = request.correlation_id.unwrap_or_else(Uuid::now_v7);
        request.correlation_id = Some(correlation_id);

        // Select provider (capability + budget checks)
        let provider = self.select_provider(&request.model, identity_id).await?;

        // Block remote model calls when safe mode is active
        if provider.provider_type == "remote" {
            if let Some(ref guard) = self.safe_mode_guard {
                guard.check_or_block("remote_model_call").await?;
            }
        }

        // Audit: log context integrity (before model call)
        if let Some(prov) = provenance {
            if let Err(e) = self
                .ledger
                .append_event(
                    None,
                    "model.context.assembled",
                    json!({
                        "action": "model.context.assembled",
                        "context_bundle_hash": prov.context_bundle_hash,
                        "total_tokens": prov.total_tokens,
                        "segment_counts": prov.segment_counts,
                        "memory_ids": prov.memory_ids,
                        "run_ids": prov.run_ids,
                        "message_ids": prov.message_ids,
                    }),
                    Some(correlation_id),
                    None,
                    Some(json!({
                        "context_bundle_hash": prov.context_bundle_hash,
                        "memory_ids": prov.memory_ids,
                        "run_ids": prov.run_ids,
                        "message_ids": prov.message_ids,
                        "total_tokens": prov.total_tokens,
                        "segment_counts": prov.segment_counts,
                    })),
                )
                .await
            {
                tracing::warn!(error = %e, "Failed to log context integrity to ledger");
            }
        }

        // Audit: log request
        if let Err(e) = self
            .ledger
            .append_event(
                Some(identity_id),
                "model.call.request",
                json!({
                    "model": request.model,
                    "provider": provider.name,
                    "correlation_id": correlation_id,
                }),
                Some(correlation_id),
                None,
                None,
            )
            .await
        {
            tracing::warn!(error = %e, "Failed to log model call request to ledger");
        }

        // POST to gateway
        let url = format!("{}/v1/complete", self.gateway_url);
        let resp = self
            .http_client
            .post(&url)
            .json(&request)
            .timeout(Duration::from_secs(120))
            .send()
            .await
            .map_err(|e| Error::GatewayUnavailable(format!("Gateway request failed: {e}")))?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::GatewayUnavailable(format!(
                "Gateway returned {status}: {body}"
            )));
        }

        let response: CompletionResponse = resp
            .json()
            .await
            .map_err(|e| Error::GatewayUnavailable(format!("Invalid gateway response: {e}")))?;

        // Persist usage
        let estimated_cost = Self::estimate_cost(&response.provider, &response.usage);
        if let Err(e) = self
            .persist_usage(
                &response.provider,
                &response.model,
                &response.usage,
                estimated_cost,
                task_id,
                run_id,
                Some(correlation_id),
            )
            .await
        {
            tracing::warn!(error = %e, "Failed to persist usage record");
        }

        // Audit: log response
        if let Err(e) = self
            .ledger
            .append_event(
                Some(identity_id),
                "model.call.response",
                json!({
                    "model": response.model,
                    "provider": response.provider,
                    "tokens_in": response.usage.prompt_tokens,
                    "tokens_out": response.usage.completion_tokens,
                    "correlation_id": correlation_id,
                }),
                Some(correlation_id),
                None,
                None,
            )
            .await
        {
            tracing::warn!(error = %e, "Failed to log model call response to ledger");
        }

        tracing::info!(
            model = %response.model,
            provider = %response.provider,
            tokens_in = response.usage.prompt_tokens,
            tokens_out = response.usage.completion_tokens,
            cost = estimated_cost,
            correlation_id = %correlation_id,
            "Completion succeeded"
        );

        Ok(response)
    }

    /// Send a streaming completion request through the gateway.
    ///
    /// Returns an async `Stream` of `CompletionChunk` items. After the stream
    /// is fully consumed, usage is estimated from accumulated content and persisted.
    ///
    /// # Arguments
    ///
    /// Same as [`complete`](Self::complete).
    pub async fn complete_stream(
        &self,
        mut request: CompletionRequest,
        identity_id: Uuid,
        task_id: Option<Uuid>,
        run_id: Option<Uuid>,
        provenance: Option<&ContextProvenance>,
    ) -> Result<impl Stream<Item = Result<CompletionChunk>>> {
        let correlation_id = request.correlation_id.unwrap_or_else(Uuid::now_v7);
        request.correlation_id = Some(correlation_id);
        request.stream = Some(true);

        // Select provider (capability + budget checks)
        let provider = self.select_provider(&request.model, identity_id).await?;

        // Block remote model calls when safe mode is active
        if provider.provider_type == "remote" {
            if let Some(ref guard) = self.safe_mode_guard {
                guard.check_or_block("remote_model_call").await?;
            }
        }

        // Audit: log context integrity (before model call)
        if let Some(prov) = provenance {
            if let Err(e) = self
                .ledger
                .append_event(
                    None,
                    "model.context.assembled",
                    json!({
                        "action": "model.context.assembled",
                        "context_bundle_hash": prov.context_bundle_hash,
                        "total_tokens": prov.total_tokens,
                        "segment_counts": prov.segment_counts,
                        "memory_ids": prov.memory_ids,
                        "run_ids": prov.run_ids,
                        "message_ids": prov.message_ids,
                    }),
                    Some(correlation_id),
                    None,
                    Some(json!({
                        "context_bundle_hash": prov.context_bundle_hash,
                        "memory_ids": prov.memory_ids,
                        "run_ids": prov.run_ids,
                        "message_ids": prov.message_ids,
                        "total_tokens": prov.total_tokens,
                        "segment_counts": prov.segment_counts,
                    })),
                )
                .await
            {
                tracing::warn!(error = %e, "Failed to log context integrity to ledger");
            }
        }

        // Audit: log request
        if let Err(e) = self
            .ledger
            .append_event(
                Some(identity_id),
                "model.call.stream_request",
                json!({
                    "model": request.model,
                    "provider": provider.name,
                    "correlation_id": correlation_id,
                }),
                Some(correlation_id),
                None,
                None,
            )
            .await
        {
            tracing::warn!(error = %e, "Failed to log stream request to ledger");
        }

        // POST to gateway streaming endpoint
        let url = format!("{}/v1/complete/stream", self.gateway_url);
        let resp = self
            .http_client
            .post(&url)
            .json(&request)
            .timeout(Duration::from_secs(300))
            .send()
            .await
            .map_err(|e| {
                Error::GatewayUnavailable(format!("Gateway stream request failed: {e}"))
            })?;

        if !resp.status().is_success() {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            return Err(Error::GatewayUnavailable(format!(
                "Gateway returned {status}: {body}"
            )));
        }

        // Parse SSE byte stream into CompletionChunk items
        let byte_stream = resp.bytes_stream();
        let model = request.model.clone();
        let provider_name = provider.name.clone();
        let pool = self.pool.clone();
        let ledger = self.ledger.clone();

        // Compute prompt character count from request messages for usage estimation
        let prompt_chars: usize = request
            .messages
            .iter()
            .map(|m| m.role.len() + m.content.len())
            .sum();

        // Accumulate content for usage estimation after stream ends
        let sse_stream = SseParser::new(byte_stream, prompt_chars);

        let mapped = sse_stream
            .then(move |chunk_result| {
                let model = model.clone();
                let provider_name = provider_name.clone();
                let pool = pool.clone();
                let ledger = ledger.clone();
                async move {
                    match chunk_result {
                        Ok(SseEvent::Chunk(chunk)) => Ok(chunk),
                        Ok(SseEvent::Done {
                            total_content,
                            model: _,
                            prompt_chars,
                        }) => {
                            // Stream finished — persist estimated usage
                            let est_prompt = (prompt_chars as i32 + 3) / 4;
                            let est_completion = (total_content.len() as i32 + 3) / 4;
                            let usage = UsageStats {
                                prompt_tokens: est_prompt,
                                completion_tokens: est_completion,
                                total_tokens: est_prompt + est_completion,
                            };
                            let cost = Self::estimate_cost(&provider_name, &usage);

                            // Best-effort persist
                            let _ = Self::persist_usage_static(
                                &pool,
                                &provider_name,
                                &model,
                                &usage,
                                cost,
                                task_id,
                                run_id,
                                Some(correlation_id),
                            )
                            .await;

                            // Best-effort ledger
                            let _ = ledger
                                .append_event(
                                    Some(identity_id),
                                    "model.call.stream_response",
                                    json!({
                                        "model": model,
                                        "provider": provider_name,
                                        "est_tokens_in": est_prompt,
                                        "est_tokens_out": est_completion,
                                        "correlation_id": correlation_id,
                                    }),
                                    Some(correlation_id),
                                    None,
                                    None,
                                )
                                .await;

                            // Return a synthetic final chunk so the caller knows the stream ended
                            Err(Error::ModelRouting("stream_done".to_string()))
                        }
                        Err(e) => Err(e),
                    }
                }
            })
            // Filter out the synthetic "stream_done" sentinel
            .filter_map(|r| async move {
                match r {
                    Err(ref e) if e.to_string().contains("stream_done") => None,
                    other => Some(other),
                }
            });

        Ok(mapped)
    }

    // =========================================================================
    // USAGE PERSISTENCE
    // =========================================================================

    /// Persist a usage record to the `usage_costs` table.
    async fn persist_usage(
        &self,
        provider_name: &str,
        model: &str,
        usage: &UsageStats,
        estimated_cost: f64,
        task_id: Option<Uuid>,
        run_id: Option<Uuid>,
        correlation_id: Option<Uuid>,
    ) -> Result<Uuid> {
        Self::persist_usage_static(
            &self.pool,
            provider_name,
            model,
            usage,
            estimated_cost,
            task_id,
            run_id,
            correlation_id,
        )
        .await
    }

    /// Static version of `persist_usage` that can be called from async closures
    /// without borrowing `self`.
    async fn persist_usage_static(
        pool: &PgPool,
        provider_name: &str,
        model: &str,
        usage: &UsageStats,
        estimated_cost: f64,
        task_id: Option<Uuid>,
        run_id: Option<Uuid>,
        correlation_id: Option<Uuid>,
    ) -> Result<Uuid> {
        // Resolve provider_id
        let provider_id: Option<Uuid> =
            sqlx::query_scalar("SELECT provider_id FROM model_providers WHERE name = $1 LIMIT 1")
                .bind(provider_name)
                .fetch_optional(pool)
                .await
                .map_err(Error::Database)?;

        let provider_id = provider_id.ok_or_else(|| {
            Error::ModelRouting(format!(
                "Unknown provider '{provider_name}' for usage persistence"
            ))
        })?;

        let usage_id: Uuid = sqlx::query_scalar(
            r"INSERT INTO usage_costs (provider_id, ts, tokens_in, tokens_out, cost_estimate, task_id, run_id, correlation_id)
              VALUES ($1, NOW(), $2, $3, $4, $5, $6, $7)
              RETURNING usage_id",
        )
        .bind(provider_id)
        .bind(usage.prompt_tokens)
        .bind(usage.completion_tokens)
        .bind(estimated_cost)
        .bind(task_id)
        .bind(run_id)
        .bind(correlation_id)
        .fetch_one(pool)
        .await
        .map_err(Error::Database)?;

        tracing::info!(
            usage_id = %usage_id,
            provider = %provider_name,
            model = %model,
            tokens_in = usage.prompt_tokens,
            tokens_out = usage.completion_tokens,
            cost = estimated_cost,
            "Usage persisted"
        );

        Ok(usage_id)
    }

    /// Rough cost estimate based on provider name and token counts.
    ///
    /// Uses approximate per-token pricing. The real cost is determined by the
    /// provider's billing; this is for budget enforcement only.
    fn estimate_cost(provider_name: &str, usage: &UsageStats) -> f64 {
        let (input_per_m, output_per_m) = match provider_name {
            "openai" => (2.50, 10.00),    // GPT-4o approximate
            "anthropic" => (3.00, 15.00), // Claude 3.5 Sonnet approximate
            "fireworks" => (0.20, 0.20),  // Fireworks serverless approximate
            _ => (0.0, 0.0),              // Local (Ollama) — free
        };

        let input_cost = f64::from(usage.prompt_tokens) * input_per_m / 1_000_000.0;
        let output_cost = f64::from(usage.completion_tokens) * output_per_m / 1_000_000.0;
        input_cost + output_cost
    }
}

// =============================================================================
// SSE STREAM PARSER
// =============================================================================

/// Internal SSE event variants produced by the parser.
#[allow(dead_code)]
enum SseEvent {
    /// A parsed completion chunk.
    Chunk(CompletionChunk),
    /// The `[DONE]` sentinel, carrying accumulated content for usage estimation.
    Done {
        total_content: String,
        model: String,
        prompt_chars: usize,
    },
}

/// Parses an SSE byte stream from the gateway into `SseEvent` items.
struct SseParser<S> {
    inner: S,
    buffer: String,
    total_content: String,
    model: String,
    prompt_chars: usize,
}

impl<S> SseParser<S> {
    fn new(inner: S, prompt_chars: usize) -> Self {
        Self {
            inner,
            buffer: String::new(),
            total_content: String::new(),
            model: String::new(),
            prompt_chars,
        }
    }
}

impl<S> Stream for SseParser<S>
where
    S: Stream<Item = std::result::Result<Bytes, reqwest::Error>> + Unpin,
{
    type Item = Result<SseEvent>;

    fn poll_next(
        self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        let this = self.get_mut();

        loop {
            // Try to extract a complete SSE event from the buffer
            if let Some(pos) = this.buffer.find("\n\n") {
                let event_text = this.buffer[..pos].to_string();
                this.buffer = this.buffer[pos + 2..].to_string();

                for line in event_text.lines() {
                    if let Some(data) = line.strip_prefix("data: ") {
                        let data = data.trim();
                        if data == "[DONE]" {
                            return std::task::Poll::Ready(Some(Ok(SseEvent::Done {
                                total_content: this.total_content.clone(),
                                model: this.model.clone(),
                                prompt_chars: this.prompt_chars,
                            })));
                        }

                        match serde_json::from_str::<CompletionChunk>(data) {
                            Ok(chunk) => {
                                if this.model.is_empty() {
                                    this.model.clone_from(&chunk.model);
                                }
                                if let Some(content) =
                                    chunk.choices.first().and_then(|c| c.delta.content.as_ref())
                                {
                                    this.total_content.push_str(content);
                                }
                                return std::task::Poll::Ready(Some(Ok(SseEvent::Chunk(chunk))));
                            }
                            Err(e) => {
                                tracing::debug!(error = %e, data = %data, "Skipping unparseable SSE data");
                            }
                        }
                    }
                }
                // Line didn't produce an event — continue loop to try next
                continue;
            }

            // Need more data from the underlying stream
            match std::pin::Pin::new(&mut this.inner).poll_next(cx) {
                std::task::Poll::Ready(Some(Ok(bytes))) => {
                    this.buffer.push_str(&String::from_utf8_lossy(&bytes));
                }
                std::task::Poll::Ready(Some(Err(e))) => {
                    return std::task::Poll::Ready(Some(Err(Error::GatewayUnavailable(format!(
                        "SSE stream error: {e}"
                    )))));
                }
                std::task::Poll::Ready(None) => {
                    // Stream ended without [DONE] — emit Done with what we have
                    if !this.total_content.is_empty() || !this.model.is_empty() {
                        let content = std::mem::take(&mut this.total_content);
                        let model = std::mem::take(&mut this.model);
                        let prompt_chars = this.prompt_chars;
                        return std::task::Poll::Ready(Some(Ok(SseEvent::Done {
                            total_content: content,
                            model,
                            prompt_chars,
                        })));
                    }
                    return std::task::Poll::Ready(None);
                }
                std::task::Poll::Pending => {
                    return std::task::Poll::Pending;
                }
            }
        }
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_match_provider_name_claude() {
        assert_eq!(
            ModelRouter::match_provider_name("claude-3-5-sonnet-20241022"),
            Some("anthropic")
        );
        assert_eq!(
            ModelRouter::match_provider_name("Claude-3-Opus"),
            Some("anthropic")
        );
    }

    #[test]
    fn test_match_provider_name_openai() {
        assert_eq!(ModelRouter::match_provider_name("gpt-4o"), Some("openai"));
        assert_eq!(
            ModelRouter::match_provider_name("gpt-4o-mini"),
            Some("openai")
        );
        assert_eq!(
            ModelRouter::match_provider_name("o1-preview"),
            Some("openai")
        );
        assert_eq!(ModelRouter::match_provider_name("o3-mini"), Some("openai"));
    }

    #[test]
    fn test_match_provider_name_fireworks() {
        assert_eq!(
            ModelRouter::match_provider_name("accounts/fireworks/models/llama-v3-70b"),
            Some("fireworks")
        );
    }

    #[test]
    fn test_match_provider_name_local() {
        assert_eq!(ModelRouter::match_provider_name("deepseek-r1:7b"), None);
        assert_eq!(ModelRouter::match_provider_name("llama3:8b"), None);
    }

    #[test]
    fn test_estimate_cost_openai() {
        let usage = UsageStats {
            prompt_tokens: 1000,
            completion_tokens: 500,
            total_tokens: 1500,
        };
        let cost = ModelRouter::estimate_cost("openai", &usage);
        // 1000 * 2.50/1M + 500 * 10.00/1M = 0.0025 + 0.005 = 0.0075
        assert!((cost - 0.0075).abs() < 1e-9);
    }

    #[test]
    fn test_estimate_cost_local() {
        let usage = UsageStats {
            prompt_tokens: 5000,
            completion_tokens: 2000,
            total_tokens: 7000,
        };
        let cost = ModelRouter::estimate_cost("ollama", &usage);
        assert!((cost - 0.0).abs() < 1e-9);
    }

    #[test]
    fn test_estimate_cost_anthropic() {
        let usage = UsageStats {
            prompt_tokens: 1_000_000,
            completion_tokens: 1_000_000,
            total_tokens: 2_000_000,
        };
        let cost = ModelRouter::estimate_cost("anthropic", &usage);
        // 1M * 3.00/1M + 1M * 15.00/1M = 3.0 + 15.0 = 18.0
        assert!((cost - 18.0).abs() < 1e-9);
    }

    #[test]
    fn test_message_serialization() {
        let msg = Message {
            role: "user".to_string(),
            content: "Hello".to_string(),
            name: None,
            tool_call_id: None,
        };
        let json = serde_json::to_value(&msg).unwrap();
        assert_eq!(json["role"], "user");
        assert_eq!(json["content"], "Hello");
        // Optional fields should be absent
        assert!(json.get("name").is_none());
        assert!(json.get("tool_call_id").is_none());
    }

    #[test]
    fn test_completion_request_serialization() {
        let req = CompletionRequest {
            model: "gpt-4o".to_string(),
            messages: vec![Message {
                role: "user".to_string(),
                content: "Hi".to_string(),
                name: None,
                tool_call_id: None,
            }],
            temperature: Some(0.7),
            max_tokens: Some(100),
            stream: None,
            correlation_id: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"model\":\"gpt-4o\""));
        assert!(json.contains("\"temperature\":0.7"));
        // stream and correlation_id should be absent
        assert!(!json.contains("\"stream\""));
        assert!(!json.contains("\"correlation_id\""));
    }
}
