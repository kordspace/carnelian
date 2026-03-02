//! Mantra subsystem — quantum-weighted context-aware reflection selection

use crate::{MagicError, Result};
use chrono::{DateTime, Timelike, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

// =============================================================================
// MantraCategory enum — 18 variants matching migration seed data
// =============================================================================

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum MantraCategory {
    CodeDevelopment,
    FinancialManagement,
    SystemHealth,
    UserOrganizationHealth,
    Communications,
    TaskBuilding,
    ScheduledJobs,
    SoulRefinement,
    MantraOptimization,
    IntegrationIdeation,
    SecurityAudit,
    MemoryKnowledge,
    CreativeExploration,
    LearningResearch,
    PerformanceOptimization,
    CollaborationDelegation,
    ReflectionIntrospection,
    InnovationExperimentation,
}

impl MantraCategory {
    pub fn as_db_name(&self) -> &str {
        match self {
            Self::CodeDevelopment => "Code Development",
            Self::FinancialManagement => "Financial Management",
            Self::SystemHealth => "System Health",
            Self::UserOrganizationHealth => "User & Organization Health",
            Self::Communications => "Communications",
            Self::TaskBuilding => "Task Building",
            Self::ScheduledJobs => "Scheduled Jobs",
            Self::SoulRefinement => "Soul Refinement",
            Self::MantraOptimization => "Mantra Optimization",
            Self::IntegrationIdeation => "Integration Ideation",
            Self::SecurityAudit => "Security & Audit",
            Self::MemoryKnowledge => "Memory & Knowledge",
            Self::CreativeExploration => "Creative Exploration",
            Self::LearningResearch => "Learning & Research",
            Self::PerformanceOptimization => "Performance Optimization",
            Self::CollaborationDelegation => "Collaboration & Delegation",
            Self::ReflectionIntrospection => "Reflection & Introspection",
            Self::InnovationExperimentation => "Innovation & Experimentation",
        }
    }

    pub fn from_db_name(s: &str) -> Option<Self> {
        match s {
            "Code Development" => Some(Self::CodeDevelopment),
            "Financial Management" => Some(Self::FinancialManagement),
            "System Health" => Some(Self::SystemHealth),
            "User & Organization Health" => Some(Self::UserOrganizationHealth),
            "Communications" => Some(Self::Communications),
            "Task Building" => Some(Self::TaskBuilding),
            "Scheduled Jobs" => Some(Self::ScheduledJobs),
            "Soul Refinement" => Some(Self::SoulRefinement),
            "Mantra Optimization" => Some(Self::MantraOptimization),
            "Integration Ideation" => Some(Self::IntegrationIdeation),
            "Security & Audit" => Some(Self::SecurityAudit),
            "Memory & Knowledge" => Some(Self::MemoryKnowledge),
            "Creative Exploration" => Some(Self::CreativeExploration),
            "Learning & Research" => Some(Self::LearningResearch),
            "Performance Optimization" => Some(Self::PerformanceOptimization),
            "Collaboration & Delegation" => Some(Self::CollaborationDelegation),
            "Reflection & Introspection" => Some(Self::ReflectionIntrospection),
            "Innovation & Experimentation" => Some(Self::InnovationExperimentation),
            _ => None,
        }
    }
}

// =============================================================================
// MantraEntry struct
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct MantraEntry {
    pub entry_id: Uuid,
    pub category_id: Uuid,
    pub text: String,
    pub use_count: i32,
    pub enabled: bool,
    pub elixir_id: Option<Uuid>,
}

// =============================================================================
// MantraContext struct
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MantraContext {
    pub recent_error_count: i64,
    pub pending_task_count: i64,
    pub idle_beats: i64,
    pub elixir_drafts_pending: i64,
    pub capability_changes_last_hour: i64,
    pub model_cost_pct: f64,
    pub sub_agents_active: i64,
    pub soul_file_age_days: i64,
    pub new_skills_last_24h: i64,
    pub magic_enabled: bool,
    pub high_latency: bool,
    pub unread_channel_messages: i64,
    pub local_hour: u8,
    pub uptime_hours: f64,
    pub elixir_quality_by_category: HashMap<String, f32>,
    pub quantum_providers_available: Vec<String>,
    pub workflow_executions_last_hour: i64,
    pub active_sessions: i64,
}

impl MantraContext {
    pub fn default_for_fallback() -> Self {
        Self {
            recent_error_count: 0,
            pending_task_count: 0,
            idle_beats: 0,
            elixir_drafts_pending: 0,
            capability_changes_last_hour: 0,
            model_cost_pct: 0.0,
            sub_agents_active: 0,
            soul_file_age_days: 0,
            new_skills_last_24h: 0,
            magic_enabled: false,
            high_latency: false,
            unread_channel_messages: 0,
            local_hour: 12,
            uptime_hours: 0.0,
            elixir_quality_by_category: HashMap::new(),
            quantum_providers_available: Vec::new(),
            workflow_executions_last_hour: 0,
            active_sessions: 0,
        }
    }
}

// =============================================================================
// MantraSelection struct
// =============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MantraSelection {
    pub category: MantraCategory,
    pub category_id: Uuid,
    pub entry_id: Uuid,
    pub mantra_text: String,
    pub system_message: String,
    pub user_message: String,
    pub entropy_source: String,
    pub selection_ts: DateTime<Utc>,
    pub suggested_skill_ids: Vec<Uuid>,
    pub elixir_reference: Option<Uuid>,
    pub context_weights: HashMap<String, i32>,
}

// =============================================================================
// MantraTree struct
// =============================================================================

pub struct MantraTree {
    // Cooldown is now per-category from DB, not a tree-level setting
}

impl MantraTree {
    pub fn new(_cooldown_beats: Option<i32>) -> Self {
        Self {}
    }

    /// Build MantraContext from database queries
    pub async fn build_context(pool: &PgPool) -> Result<MantraContext> {
        let mut tx = pool.begin().await?;

        let recent_error_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM tasks WHERE state = 'failed' AND updated_at > NOW() - INTERVAL '30 minutes'"
        )
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or(0);

        let pending_task_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM tasks WHERE state = 'pending'"
        )
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or(0);

        let idle_beats: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM heartbeat_history WHERE status = 'ok' AND ts > NOW() - INTERVAL '30 minutes'"
        )
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or(0);

        let elixir_drafts_pending: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM elixir_drafts WHERE status = 'pending'"
        )
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or(0);

        let capability_changes_last_hour: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM capability_grants WHERE created_at > NOW() - INTERVAL '1 hour'"
        )
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or(0);

        let sub_agents_active: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sub_agents WHERE terminated_at IS NULL"
        )
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or(0);

        let soul_file_age_days: i64 = sqlx::query_scalar(
            "SELECT COALESCE(EXTRACT(EPOCH FROM (NOW() - updated_at)) / 86400, 9999)::bigint FROM identities WHERE name = 'Lian' AND identity_type = 'core' LIMIT 1"
        )
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or(9999);

        let new_skills_last_24h: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM skills WHERE discovered_at > NOW() - INTERVAL '24 hours'"
        )
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or(0);

        let unread_channel_messages: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM session_messages WHERE role = 'user' AND ts > NOW() - INTERVAL '1 hour'"
        )
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or(0);

        let workflow_executions_last_hour: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM tasks WHERE skill_id IN (SELECT skill_id FROM skills WHERE name LIKE 'workflow-%') AND created_at > NOW() - INTERVAL '1 hour'"
        )
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or(0);

        let elixir_quality_rows: Vec<(String, f32)> = sqlx::query_as(
            "SELECT elixir_type, AVG(quality_score)::real FROM elixirs WHERE quality_score IS NOT NULL GROUP BY elixir_type"
        )
        .fetch_all(&mut *tx)
        .await?;

        let elixir_quality_by_category: HashMap<String, f32> = elixir_quality_rows.into_iter().collect();

        let active_sessions: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM sessions WHERE expires_at > NOW()"
        )
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or(0);

        // Compute uptime from first heartbeat
        let uptime_hours: f64 = sqlx::query_scalar(
            "SELECT COALESCE(EXTRACT(EPOCH FROM (NOW() - MIN(ts))) / 3600, 0.0) FROM heartbeat_history"
        )
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or(0.0);

        // Compute model cost percentage from recent usage
        let model_cost_pct: f64 = sqlx::query_scalar(
            "SELECT COALESCE(SUM(input_tokens + output_tokens) * 100.0 / NULLIF(1000000, 0), 0.0) FROM model_usage WHERE created_at > NOW() - INTERVAL '1 hour'"
        )
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or(0.0);

        // Check for high latency from recent heartbeats
        let avg_latency_ms: f64 = sqlx::query_scalar(
            "SELECT COALESCE(AVG(EXTRACT(EPOCH FROM (ts - LAG(ts) OVER (ORDER BY ts))) * 1000), 0.0) FROM heartbeat_history WHERE ts > NOW() - INTERVAL '5 minutes'"
        )
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or(0.0);
        let high_latency = avg_latency_ms > 5000.0;

        tx.commit().await?;

        let local_hour = chrono::Local::now().hour() as u8;

        Ok(MantraContext {
            recent_error_count,
            pending_task_count,
            idle_beats,
            elixir_drafts_pending,
            capability_changes_last_hour,
            model_cost_pct,
            sub_agents_active,
            soul_file_age_days,
            new_skills_last_24h,
            magic_enabled: true,
            high_latency,
            unread_channel_messages,
            local_hour,
            uptime_hours,
            elixir_quality_by_category,
            quantum_providers_available: vec!["os".into()],
            workflow_executions_last_hour,
            active_sessions,
        })
    }

    /// Select a mantra using quantum entropy and context weights (consumer-facing API)
    pub async fn select(
        &self,
        _entropy: &[u8],
        _context: &MantraContext,
    ) -> Result<MantraSelection> {
        // This method requires preloaded category and entry data
        // For now, return an error directing to use select_with_pool
        Err(MagicError::EntropyUnavailable(
            "Use select_with_pool for DB-backed selection".into(),
        ))
    }

    /// Select a mantra using quantum entropy and context weights with DB access
    pub async fn select_with_pool(
        &self,
        entropy: &[u8],
        context: &MantraContext,
        pool: &PgPool,
    ) -> Result<MantraSelection> {
        if entropy.len() < 8 {
            return Err(MagicError::EntropyUnavailable(
                "Need at least 8 bytes of entropy".into(),
            ));
        }

        // Fetch all enabled categories
        let categories: Vec<(Uuid, String, i32, i32, String, String)> = sqlx::query_as(
            "SELECT category_id, name, base_weight, cooldown_beats, system_message, user_message 
             FROM mantra_categories WHERE enabled = true"
        )
        .fetch_all(pool)
        .await?;

        if categories.is_empty() {
            return Err(MagicError::EntropyUnavailable("No enabled mantra categories".into()));
        }

        // Fetch recently used category_ids with their usage order
        // We need to check per-category cooldown, so fetch more history
        let max_cooldown = categories.iter().map(|(_, _, _, cd, _, _)| *cd).max().unwrap_or(3);
        let recent_history: Vec<(Uuid, i64)> = sqlx::query_as(
            "SELECT category_id, ROW_NUMBER() OVER (ORDER BY ts DESC) as position FROM mantra_history ORDER BY ts DESC LIMIT $1"
        )
        .bind(max_cooldown)
        .fetch_all(pool)
        .await?;

        // Build a map of category_id -> most recent position (1-indexed)
        let mut category_last_used: HashMap<Uuid, i64> = HashMap::new();
        for (cat_id, position) in recent_history {
            category_last_used.entry(cat_id).or_insert(position);
        }

        // Compute weights
        let weights = self.compute_weights(context, &categories, &category_last_used);

        // Weighted pick for category
        let (selected_cat_id, selected_cat_name, system_msg, user_msg) = 
            self.weighted_pick(entropy, &categories, &weights)?;

        // Fetch enabled entries for selected category
        let entry_rows: Vec<(Uuid, Uuid, String, i32, bool, Option<Uuid>)> = sqlx::query_as(
            "SELECT entry_id, category_id, text, use_count, enabled, elixir_id 
             FROM mantra_entries WHERE category_id = $1 AND enabled = true"
        )
        .bind(selected_cat_id)
        .fetch_all(pool)
        .await?;

        let entries: Vec<MantraEntry> = entry_rows
            .into_iter()
            .map(|(entry_id, category_id, text, use_count, enabled, elixir_id)| MantraEntry {
                entry_id,
                category_id,
                text,
                use_count,
                enabled,
                elixir_id,
            })
            .collect();

        if entries.is_empty() {
            return Err(MagicError::EntropyUnavailable(
                format!("No enabled entries for category {}", selected_cat_name)
            ));
        }

        // Inverse frequency pick for entry
        let selected_entry = self.inverse_freq_pick(&entropy[4..8], &entries)?;

        // Get skill suggestions
        let suggested_skill_ids = Self::get_skill_suggestions(selected_cat_id, pool).await?;

        // Resolve templates
        let system_message = Self::resolve_template(&system_msg, &selected_entry.text, context);
        let user_message = Self::resolve_template(&user_msg, &selected_entry.text, context);

        let category = MantraCategory::from_db_name(&selected_cat_name)
            .ok_or_else(|| MagicError::EntropyUnavailable(format!("Unknown category: {}", selected_cat_name)))?;

        Ok(MantraSelection {
            category,
            category_id: selected_cat_id,
            entry_id: selected_entry.entry_id,
            mantra_text: selected_entry.text.clone(),
            system_message,
            user_message,
            entropy_source: "quantum".into(),
            selection_ts: Utc::now(),
            suggested_skill_ids,
            elixir_reference: selected_entry.elixir_id,
            context_weights: weights,
        })
    }

    fn compute_weights(
        &self,
        context: &MantraContext,
        categories: &[(Uuid, String, i32, i32, String, String)],
        category_last_used: &HashMap<Uuid, i64>,
    ) -> HashMap<String, i32> {
        let mut weights = HashMap::new();

        for (cat_id, name, base_weight, cooldown_beats, _, _) in categories {
            let mut weight = *base_weight;

            // Apply context bonuses based on category
            match name.as_str() {
                "System Health" if context.recent_error_count > 3 => weight += 3,
                "Financial Management" if context.model_cost_pct > 80.0 => weight += 3,
                "Task Building" if context.pending_task_count > 10 => weight += 2,
                "Communications" if context.unread_channel_messages > 5 => weight += 2,
                "Soul Refinement" if context.soul_file_age_days > 7 => weight += 3,
                "Code Development" if context.new_skills_last_24h > 0 => weight += 1,
                "Security & Audit" if context.capability_changes_last_hour > 0 => weight += 2,
                "Performance Optimization" if context.high_latency => weight += 3,
                "Reflection & Introspection" if context.local_hour >= 22 || context.local_hour <= 6 => weight += 2,
                "Innovation & Experimentation" if context.magic_enabled => weight += 1,
                _ => {}
            }

            // Apply per-category cooldown enforcement
            if let Some(&last_position) = category_last_used.get(cat_id) {
                if last_position <= *cooldown_beats as i64 {
                    weight = 0;
                }
            }

            weights.insert(name.clone(), weight);
        }

        // If all weights are zero, reset to base weights
        if weights.values().all(|&w| w == 0) {
            for (_, name, base_weight, _, _, _) in categories {
                weights.insert(name.clone(), *base_weight);
            }
        }

        weights
    }

    fn weighted_pick(
        &self,
        entropy: &[u8],
        categories: &[(Uuid, String, i32, i32, String, String)],
        weights: &HashMap<String, i32>,
    ) -> Result<(Uuid, String, String, String)> {
        let total_weight: i32 = weights.values().sum();
        if total_weight == 0 {
            return Err(MagicError::EntropyUnavailable("All weights are zero".into()));
        }

        let entropy_val = u32::from_le_bytes([entropy[0], entropy[1], entropy[2], entropy[3]]);
        let pick = (entropy_val % total_weight as u32) as i32;

        let mut cumulative = 0;
        for (cat_id, name, _, _, sys_msg, user_msg) in categories {
            let weight = weights.get(name).copied().unwrap_or(0);
            cumulative += weight;
            if pick < cumulative {
                return Ok((*cat_id, name.clone(), sys_msg.clone(), user_msg.clone()));
            }
        }

        // Fallback to first category
        let (cat_id, name, _, _, sys_msg, user_msg) = &categories[0];
        Ok((*cat_id, name.clone(), sys_msg.clone(), user_msg.clone()))
    }

    fn inverse_freq_pick(&self, entropy: &[u8], entries: &[MantraEntry]) -> Result<MantraEntry> {
        let weights: Vec<f64> = entries
            .iter()
            .map(|e| 1.0 / (e.use_count as f64 + 1.0))
            .collect();

        let total_weight: f64 = weights.iter().sum();
        if total_weight == 0.0 {
            return Err(MagicError::EntropyUnavailable("All entry weights are zero".into()));
        }

        let entropy_val = u32::from_le_bytes([entropy[0], entropy[1], entropy[2], entropy[3]]);
        let pick = (entropy_val as f64 / u32::MAX as f64) * total_weight;

        let mut cumulative = 0.0;
        for (idx, weight) in weights.iter().enumerate() {
            cumulative += weight;
            if pick < cumulative {
                return Ok(entries[idx].clone());
            }
        }

        // Fallback to first entry
        Ok(entries[0].clone())
    }

    fn resolve_template(template: &str, mantra_text: &str, context: &MantraContext) -> String {
        template
            .replace("{mantra_text}", mantra_text)
            .replace("{tasks_queued}", &context.pending_task_count.to_string())
            .replace("{recent_error_count}", &context.recent_error_count.to_string())
            .replace("{idle_beats}", &context.idle_beats.to_string())
            .replace("{elixir_drafts_pending}", &context.elixir_drafts_pending.to_string())
            .replace("{capability_changes_last_hour}", &context.capability_changes_last_hour.to_string())
            .replace("{model_cost_pct}", &format!("{:.1}", context.model_cost_pct))
            .replace("{sub_agents_active}", &context.sub_agents_active.to_string())
            .replace("{soul_file_age_days}", &context.soul_file_age_days.to_string())
            .replace("{new_skills_last_24h}", &context.new_skills_last_24h.to_string())
            .replace("{magic_enabled}", &context.magic_enabled.to_string())
            .replace("{high_latency}", &context.high_latency.to_string())
            .replace("{unread_channel_messages}", &context.unread_channel_messages.to_string())
            .replace("{local_hour}", &context.local_hour.to_string())
            .replace("{uptime_hours}", &format!("{:.1}", context.uptime_hours))
            .replace("{workflow_executions_last_hour}", &context.workflow_executions_last_hour.to_string())
            .replace("{active_sessions}", &context.active_sessions.to_string())
    }

    async fn get_skill_suggestions(category_id: Uuid, pool: &PgPool) -> Result<Vec<Uuid>> {
        let skill_ids: Vec<Uuid> = sqlx::query_scalar(
            "SELECT s.skill_id 
             FROM skills s
             JOIN (SELECT unnest(suggested_skill_tags) AS tag 
                   FROM mantra_categories WHERE category_id = $1) tags ON s.name = tags.tag
             WHERE s.enabled = true"
        )
        .bind(category_id)
        .fetch_all(pool)
        .await?;

        Ok(skill_ids)
    }
}

impl Default for MantraTree {
    fn default() -> Self {
        Self::new(None)
    }
}

// =============================================================================
// Unit tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_weights_system_health_boost() {
        let tree = MantraTree::new(None);
        let mut context = MantraContext::default_for_fallback();
        context.recent_error_count = 5;

        let categories = vec![
            (Uuid::new_v4(), "System Health".into(), 1, 3, "".into(), "".into()),
            (Uuid::new_v4(), "Code Development".into(), 1, 3, "".into(), "".into()),
        ];

        let category_last_used = HashMap::new();
        let weights = tree.compute_weights(&context, &categories, &category_last_used);

        assert!(weights.get("System Health").copied().unwrap_or(0) >= 4);
        assert_eq!(weights.get("Code Development").copied().unwrap_or(0), 1);
    }

    #[test]
    fn test_cooldown_zeroes_weight() {
        let tree = MantraTree::new(None);
        let context = MantraContext::default_for_fallback();

        let cat_id = Uuid::new_v4();
        let categories = vec![
            (cat_id, "System Health".into(), 5, 3, "".into(), "".into()),
            (Uuid::new_v4(), "Code Development".into(), 1, 3, "".into(), "".into()),
        ];

        let mut category_last_used = HashMap::new();
        category_last_used.insert(cat_id, 1); // Used in position 1 (most recent)
        let weights = tree.compute_weights(&context, &categories, &category_last_used);

        assert_eq!(weights.get("System Health").copied().unwrap_or(0), 0);
        assert_eq!(weights.get("Code Development").copied().unwrap_or(0), 1);
    }

    #[test]
    fn test_weighted_pick_distribution() {
        let tree = MantraTree::new(None);
        let cat1 = Uuid::new_v4();
        let cat2 = Uuid::new_v4();

        let categories = vec![
            (cat1, "High Weight".into(), 10, 3, "sys1".into(), "user1".into()),
            (cat2, "Low Weight".into(), 1, 3, "sys2".into(), "user2".into()),
        ];

        let mut weights = HashMap::new();
        weights.insert("High Weight".into(), 10);
        weights.insert("Low Weight".into(), 1);

        let mut high_count = 0;
        let mut low_count = 0;

        for i in 0..1000 {
            let entropy = [(i % 256) as u8, ((i / 256) % 256) as u8, 0, 0];
            if let Ok((picked_id, _, _, _)) = tree.weighted_pick(&entropy, &categories, &weights) {
                if picked_id == cat1 {
                    high_count += 1;
                } else {
                    low_count += 1;
                }
            }
        }

        assert!(high_count > low_count * 5);
    }

    #[test]
    fn test_inverse_freq_pick_favors_low_use_count() {
        let tree = MantraTree::new(None);
        let entries = vec![
            MantraEntry {
                entry_id: Uuid::new_v4(),
                category_id: Uuid::new_v4(),
                text: "Low use".into(),
                use_count: 0,
                enabled: true,
                elixir_id: None,
            },
            MantraEntry {
                entry_id: Uuid::new_v4(),
                category_id: Uuid::new_v4(),
                text: "Medium use".into(),
                use_count: 5,
                enabled: true,
                elixir_id: None,
            },
            MantraEntry {
                entry_id: Uuid::new_v4(),
                category_id: Uuid::new_v4(),
                text: "High use".into(),
                use_count: 50,
                enabled: true,
                elixir_id: None,
            },
        ];

        let mut low_count = 0;
        for i in 0..1000 {
            let entropy = [(i % 256) as u8, ((i / 256) % 256) as u8, 0, 0];
            if let Ok(entry) = tree.inverse_freq_pick(&entropy, &entries) {
                if entry.use_count == 0 {
                    low_count += 1;
                }
            }
        }

        assert!(low_count > 700);
    }

    #[test]
    fn test_resolve_template_substitutes_all_variables() {
        let mut context = MantraContext::default_for_fallback();
        context.pending_task_count = 42;
        context.recent_error_count = 7;
        context.local_hour = 14;

        let template = "Tasks: {tasks_queued}, Errors: {recent_error_count}, Hour: {local_hour}, Mantra: {mantra_text}";
        let result = MantraTree::resolve_template(template, "Test mantra", &context);

        assert_eq!(result, "Tasks: 42, Errors: 7, Hour: 14, Mantra: Test mantra");
        assert!(!result.contains('{'));
    }

    #[test]
    fn test_mantra_category_roundtrip() {
        let categories = vec![
            MantraCategory::CodeDevelopment,
            MantraCategory::FinancialManagement,
            MantraCategory::SystemHealth,
            MantraCategory::UserOrganizationHealth,
            MantraCategory::Communications,
            MantraCategory::TaskBuilding,
            MantraCategory::ScheduledJobs,
            MantraCategory::SoulRefinement,
            MantraCategory::MantraOptimization,
            MantraCategory::IntegrationIdeation,
            MantraCategory::SecurityAudit,
            MantraCategory::MemoryKnowledge,
            MantraCategory::CreativeExploration,
            MantraCategory::LearningResearch,
            MantraCategory::PerformanceOptimization,
            MantraCategory::CollaborationDelegation,
            MantraCategory::ReflectionIntrospection,
            MantraCategory::InnovationExperimentation,
        ];

        for cat in categories {
            let db_name = cat.as_db_name();
            let roundtrip = MantraCategory::from_db_name(db_name);
            assert_eq!(roundtrip, Some(cat));
        }
    }
}
