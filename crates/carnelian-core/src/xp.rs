//! Experience point (XP) system for agent progression
//!
//! Manages XP earning, level progression, skill metrics tracking, and quality
//! bonuses. All XP data is persisted in the `agent_xp`, `xp_events`,
//! `skill_metrics`, and `level_progression` tables.
//!
//! # XP Sources
//!
//! - **Task completion**: XP scaled by task duration
//! - **Ledger signing**: XP for privileged action signatures, scaled by risk
//! - **Skill usage**: First-use bonus + ongoing skill metric tracking
//! - **Quality bonus**: Daily cron awards for high success rates, zero errors, fast execution
//!
//! # Level Curve
//!
//! Levels follow the 1.172 exponent curve defined in `level_progression` (migration 0004).
//! `award_xp` automatically checks for level-ups after each XP grant.

use carnelian_common::{Error, Result};
use serde_json::Value as JsonValue;
use sqlx::PgPool;
use uuid::Uuid;

// =============================================================================
// XP SOURCE
// =============================================================================

/// Describes the origin of an XP award.
#[derive(Debug, Clone)]
pub enum XpSource {
    /// XP earned from completing a task.
    TaskCompletion {
        task_id: Uuid,
        skill_id: Option<Uuid>,
    },
    /// XP earned from signing a privileged ledger event.
    LedgerSigning { ledger_event_id: i64 },
    /// XP earned from using a skill (first-use bonus).
    SkillUsage { skill_id: Uuid },
    /// XP earned from a periodic quality bonus check.
    QualityBonus,
}

impl XpSource {
    /// Map to the DB `source` column value.
    pub fn to_source_str(&self) -> &'static str {
        match self {
            XpSource::TaskCompletion { .. } => "task_completion",
            XpSource::LedgerSigning { .. } => "ledger_signing",
            XpSource::SkillUsage { .. } => "skill_usage",
            XpSource::QualityBonus => "quality_bonus",
        }
    }
}

// =============================================================================
// XP MANAGER
// =============================================================================

/// Owns all XP logic: awarding, level-up detection, skill metrics, quality bonuses.
pub struct XpManager {
    pool: PgPool,
}

impl XpManager {
    /// Create a new `XpManager` backed by the given connection pool.
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // =========================================================================
    // PURE CALCULATION HELPERS
    // =========================================================================

    /// Calculate XP for a completed task based on its duration.
    ///
    /// | Duration           | XP |
    /// |--------------------|----|
    /// | < 1 min            |  5 |
    /// | 1 – 5 min          | 15 |
    /// | > 5 min            | 30 |
    pub fn calculate_task_xp(duration_ms: i64) -> i32 {
        if duration_ms < 60_000 {
            5
        } else if duration_ms <= 300_000 {
            15
        } else {
            30
        }
    }

    /// Calculate XP for signing a privileged ledger event based on risk tier.
    ///
    /// | Action types                                                        | Risk   | XP |
    /// |---------------------------------------------------------------------|--------|----|
    /// | `capability.grant/revoke`, `approval.granted/denied`                | Medium | 25 |
    /// | `db.migration`, `exec.shell`, `worker.quarantined`                  | High   | 50 |
    /// | All other privileged (`config.change`, `safe_mode.*`)               | Low    | 10 |
    pub fn calculate_ledger_xp(action_type: &str) -> i32 {
        match action_type {
            "capability.grant" | "capability.revoke" | "approval.granted" | "approval.denied" => 25,
            "db.migration" | "exec.shell" | "worker.quarantined" => 50,
            _ => 10,
        }
    }

    // =========================================================================
    // AGENT XP BOOTSTRAP
    // =========================================================================

    /// Ensure an `agent_xp` row exists for the given identity.
    ///
    /// Uses `ON CONFLICT DO NOTHING` for idempotency. Fetches the initial
    /// `xp_to_next_level` from `level_progression WHERE level = 2`.
    pub async fn ensure_agent_xp(&self, identity_id: Uuid) -> Result<()> {
        let xp_to_next: Option<i64> =
            sqlx::query_scalar("SELECT total_xp_required FROM level_progression WHERE level = 2")
                .fetch_optional(&self.pool)
                .await
                .map_err(Error::Database)?;

        let xp_to_next = xp_to_next.unwrap_or(100);

        sqlx::query(
            "INSERT INTO agent_xp (identity_id, total_xp, level, xp_to_next_level) \
             VALUES ($1, 0, 1, $2) \
             ON CONFLICT (identity_id) DO NOTHING",
        )
        .bind(identity_id)
        .bind(xp_to_next)
        .execute(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(())
    }

    // =========================================================================
    // AWARD XP
    // =========================================================================

    /// Award XP to an identity and check for level-up.
    ///
    /// Returns `Ok(Some(new_level))` if the agent leveled up, `Ok(None)` otherwise.
    ///
    /// Transactional sequence:
    /// 1. Insert `xp_events` row
    /// 2. Update `agent_xp.total_xp`
    /// 3. Query `level_progression` for new level
    /// 4. If leveled up, update `agent_xp.level` and `xp_to_next_level`
    pub async fn award_xp(
        &self,
        identity_id: Uuid,
        source: XpSource,
        xp_amount: i32,
        metadata: Option<JsonValue>,
    ) -> Result<Option<i32>> {
        if xp_amount <= 0 {
            return Ok(None);
        }

        let source_str = source.to_source_str();
        let (task_id, skill_id, ledger_event_id) = match &source {
            XpSource::TaskCompletion { task_id, skill_id } => {
                (Some(*task_id), *skill_id, None::<i64>)
            }
            XpSource::LedgerSigning { ledger_event_id } => (None, None, Some(*ledger_event_id)),
            XpSource::SkillUsage { skill_id } => (None, Some(*skill_id), None),
            XpSource::QualityBonus => (None, None, None),
        };

        let metadata_value = metadata.unwrap_or_else(|| serde_json::json!({}));

        // 1. Insert xp_events row
        sqlx::query(
            "INSERT INTO xp_events (identity_id, source, xp_amount, task_id, skill_id, ledger_event_id, metadata) \
             VALUES ($1, $2, $3, $4, $5, $6, $7)",
        )
        .bind(identity_id)
        .bind(source_str)
        .bind(xp_amount)
        .bind(task_id)
        .bind(skill_id)
        .bind(ledger_event_id)
        .bind(&metadata_value)
        .execute(&self.pool)
        .await
        .map_err(Error::Database)?;

        // 2. Update agent_xp total
        let row: Option<(i64, i32)> = sqlx::query_as(
            "UPDATE agent_xp SET total_xp = total_xp + $2, updated_at = NOW() \
             WHERE identity_id = $1 \
             RETURNING total_xp, level",
        )
        .bind(identity_id)
        .bind(i64::from(xp_amount))
        .fetch_optional(&self.pool)
        .await
        .map_err(Error::Database)?;

        let (new_total_xp, current_level) = match row {
            Some(r) => r,
            None => {
                tracing::warn!(
                    identity_id = %identity_id,
                    "No agent_xp row found for identity, skipping level check"
                );
                return Ok(None);
            }
        };

        // 3. Determine new level from level_progression
        let new_level: Option<i32> = sqlx::query_scalar(
            "SELECT level FROM level_progression \
             WHERE total_xp_required <= $1 \
             ORDER BY level DESC LIMIT 1",
        )
        .bind(new_total_xp)
        .fetch_optional(&self.pool)
        .await
        .map_err(Error::Database)?;

        let new_level = new_level.unwrap_or(1);

        // 4. Level-up check
        if new_level > current_level {
            // Get XP required for the *next* level after the new one
            let next_level_xp: Option<i64> = sqlx::query_scalar(
                "SELECT total_xp_required FROM level_progression WHERE level = $1",
            )
            .bind(new_level + 1)
            .fetch_optional(&self.pool)
            .await
            .map_err(Error::Database)?;

            let xp_to_next = next_level_xp
                .map(|req| req - new_total_xp)
                .unwrap_or(0)
                .max(0);

            sqlx::query(
                "UPDATE agent_xp SET level = $2, xp_to_next_level = $3, updated_at = NOW() \
                 WHERE identity_id = $1",
            )
            .bind(identity_id)
            .bind(new_level)
            .bind(xp_to_next)
            .execute(&self.pool)
            .await
            .map_err(Error::Database)?;

            tracing::info!(
                identity_id = %identity_id,
                old_level = current_level,
                new_level = new_level,
                total_xp = new_total_xp,
                "Agent leveled up!"
            );

            return Ok(Some(new_level));
        }

        Ok(None)
    }

    // =========================================================================
    // SKILL TRACKING
    // =========================================================================

    /// Check whether this is the first time the identity has used the given skill.
    pub async fn is_first_skill_use(&self, identity_id: Uuid, skill_id: Uuid) -> Result<bool> {
        let exists: Option<i64> = sqlx::query_scalar(
            "SELECT xp_event_id FROM xp_events \
             WHERE identity_id = $1 AND skill_id = $2 \
             LIMIT 1",
        )
        .bind(identity_id)
        .bind(skill_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(Error::Database)?;

        Ok(exists.is_none())
    }

    /// Upsert skill metrics after a task execution.
    ///
    /// Increments counters, recalculates averages, and updates skill-level XP.
    pub async fn update_skill_metrics(
        &self,
        skill_id: Uuid,
        duration_ms: i64,
        success: bool,
    ) -> Result<()> {
        let xp_earned = if success {
            i64::from(Self::calculate_task_xp(duration_ms))
        } else {
            0
        };

        // Upsert with all metric updates in a single statement
        sqlx::query(
            r"INSERT INTO skill_metrics (skill_id, usage_count, success_count, failure_count,
                                         total_duration_ms, avg_duration_ms, total_xp_earned,
                                         skill_level, last_used_at)
              VALUES ($1, 1,
                      CASE WHEN $2 THEN 1 ELSE 0 END,
                      CASE WHEN $2 THEN 0 ELSE 1 END,
                      $3, $3, $4, 1, NOW())
              ON CONFLICT (skill_id) DO UPDATE SET
                usage_count = skill_metrics.usage_count + 1,
                success_count = skill_metrics.success_count + CASE WHEN $2 THEN 1 ELSE 0 END,
                failure_count = skill_metrics.failure_count + CASE WHEN $2 THEN 0 ELSE 1 END,
                total_duration_ms = skill_metrics.total_duration_ms + $3,
                avg_duration_ms = (skill_metrics.total_duration_ms + $3) / (skill_metrics.usage_count + 1),
                total_xp_earned = skill_metrics.total_xp_earned + $4,
                last_used_at = NOW()",
        )
        .bind(skill_id)
        .bind(success)
        .bind(duration_ms)
        .bind(xp_earned)
        .execute(&self.pool)
        .await
        .map_err(Error::Database)?;

        // Recalculate skill_level from level_progression
        let total_xp: Option<i64> =
            sqlx::query_scalar("SELECT total_xp_earned FROM skill_metrics WHERE skill_id = $1")
                .bind(skill_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(Error::Database)?;

        if let Some(total_xp) = total_xp {
            let skill_level: Option<i32> = sqlx::query_scalar(
                "SELECT level FROM level_progression \
                 WHERE total_xp_required <= $1 \
                 ORDER BY level DESC LIMIT 1",
            )
            .bind(total_xp)
            .fetch_optional(&self.pool)
            .await
            .map_err(Error::Database)?;

            if let Some(level) = skill_level {
                sqlx::query("UPDATE skill_metrics SET skill_level = $2 WHERE skill_id = $1")
                    .bind(skill_id)
                    .bind(level)
                    .execute(&self.pool)
                    .await
                    .map_err(Error::Database)?;
            }
        }

        Ok(())
    }

    // =========================================================================
    // QUALITY BONUS CRON
    // =========================================================================

    /// Run the daily quality bonus check for all active agents.
    ///
    /// Awards bonuses for:
    /// 1. **Success rate bonus** (+5% XP): Skills with > 95% success rate
    /// 2. **Zero errors in 24h** (+50 XP): No failed tasks in the last 24 hours
    /// 3. **Fast execution bonus** (+10% XP): Below-average task duration
    pub async fn run_quality_bonus_check(&self, pool: &PgPool) -> Result<()> {
        // Get all identities with agent_xp rows
        let agents: Vec<(Uuid, i64)> = sqlx::query_as("SELECT identity_id, total_xp FROM agent_xp")
            .fetch_all(pool)
            .await
            .map_err(Error::Database)?;

        if agents.is_empty() {
            tracing::debug!("No agents with XP rows, skipping quality bonus check");
            return Ok(());
        }

        tracing::info!(agent_count = agents.len(), "Running quality bonus check");

        for (identity_id, current_xp) in &agents {
            // 1. Success rate bonus: per-agent success ratio > 95%
            let agent_stats: Option<(i64, i64)> = sqlx::query_as(
                r"SELECT COUNT(*) AS usage_count,
                         COUNT(*) FILTER (WHERE tr.state = 'success') AS success_count
                  FROM task_runs tr
                  JOIN tasks t ON t.task_id = tr.task_id
                  WHERE t.assigned_to = $1
                    AND tr.ended_at IS NOT NULL",
            )
            .bind(identity_id)
            .fetch_optional(pool)
            .await
            .map_err(Error::Database)?;

            let qualifies = agent_stats
                .map(|(usage, success)| usage > 0 && (success as f64 / usage as f64) > 0.95)
                .unwrap_or(false);

            if qualifies {
                let bonus = ((*current_xp as f64) * 0.05) as i32;
                let bonus = bonus.min(500).max(1); // cap at 500, min 1
                if let Err(e) = self
                    .award_xp(
                        *identity_id,
                        XpSource::QualityBonus,
                        bonus,
                        Some(serde_json::json!({
                            "bonus_type": "success_rate",
                            "agent_usage_count": agent_stats.map(|(u, _)| u),
                            "agent_success_count": agent_stats.map(|(_, s)| s),
                            "modifier": "5%",
                        })),
                    )
                    .await
                {
                    tracing::warn!(
                        identity_id = %identity_id,
                        error = %e,
                        "Failed to award success rate bonus"
                    );
                }
            }

            // 2. Zero errors in 24h bonus
            let failed_count: Option<i64> = sqlx::query_scalar(
                r"SELECT COUNT(*) FROM task_runs tr
                  JOIN tasks t ON t.task_id = tr.task_id
                  WHERE t.assigned_to = $1
                    AND tr.ended_at > NOW() - INTERVAL '24 hours'
                    AND tr.state = 'failed'",
            )
            .bind(identity_id)
            .fetch_one(pool)
            .await
            .map_err(Error::Database)?;

            let total_runs: Option<i64> = sqlx::query_scalar(
                r"SELECT COUNT(*) FROM task_runs tr
                  JOIN tasks t ON t.task_id = tr.task_id
                  WHERE t.assigned_to = $1
                    AND tr.ended_at > NOW() - INTERVAL '24 hours'",
            )
            .bind(identity_id)
            .fetch_one(pool)
            .await
            .map_err(Error::Database)?;

            if failed_count.unwrap_or(0) == 0 && total_runs.unwrap_or(0) > 0 {
                if let Err(e) = self
                    .award_xp(
                        *identity_id,
                        XpSource::QualityBonus,
                        50,
                        Some(serde_json::json!({
                            "bonus_type": "zero_errors_24h",
                            "total_runs_24h": total_runs,
                        })),
                    )
                    .await
                {
                    tracing::warn!(
                        identity_id = %identity_id,
                        error = %e,
                        "Failed to award zero-errors bonus"
                    );
                }
            }

            // 3. Fast execution bonus: agent's avg duration below global average
            let agent_avg: Option<f64> = sqlx::query_scalar(
                r"SELECT AVG(EXTRACT(EPOCH FROM (tr.ended_at - tr.started_at)) * 1000)
                  FROM task_runs tr
                  JOIN tasks t ON t.task_id = tr.task_id
                  WHERE t.assigned_to = $1
                    AND tr.ended_at > NOW() - INTERVAL '24 hours'
                    AND tr.state = 'success'",
            )
            .bind(identity_id)
            .fetch_one(pool)
            .await
            .map_err(Error::Database)?;

            let global_avg: Option<f64> = sqlx::query_scalar(
                r"SELECT AVG(EXTRACT(EPOCH FROM (ended_at - started_at)) * 1000)
                  FROM task_runs
                  WHERE ended_at > NOW() - INTERVAL '24 hours'
                    AND state = 'success'",
            )
            .fetch_one(pool)
            .await
            .map_err(Error::Database)?;

            if let (Some(a_avg), Some(g_avg)) = (agent_avg, global_avg) {
                if a_avg < g_avg && g_avg > 0.0 {
                    let bonus = ((*current_xp as f64) * 0.10) as i32;
                    let bonus = bonus.min(500).max(1);
                    if let Err(e) = self
                        .award_xp(
                            *identity_id,
                            XpSource::QualityBonus,
                            bonus,
                            Some(serde_json::json!({
                                "bonus_type": "fast_execution",
                                "agent_avg_ms": a_avg,
                                "global_avg_ms": g_avg,
                                "modifier": "10%",
                            })),
                        )
                        .await
                    {
                        tracing::warn!(
                            identity_id = %identity_id,
                            error = %e,
                            "Failed to award fast execution bonus"
                        );
                    }
                }
            }
        }

        tracing::info!("Quality bonus check completed");
        Ok(())
    }
}

// =============================================================================
// TESTS
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_task_xp_tiers() {
        // < 1 min → 5 XP
        assert_eq!(XpManager::calculate_task_xp(0), 5);
        assert_eq!(XpManager::calculate_task_xp(30_000), 5);
        assert_eq!(XpManager::calculate_task_xp(59_999), 5);

        // 1–5 min → 15 XP
        assert_eq!(XpManager::calculate_task_xp(60_000), 15);
        assert_eq!(XpManager::calculate_task_xp(180_000), 15);
        assert_eq!(XpManager::calculate_task_xp(300_000), 15);

        // > 5 min → 30 XP
        assert_eq!(XpManager::calculate_task_xp(300_001), 30);
        assert_eq!(XpManager::calculate_task_xp(600_000), 30);
    }

    #[test]
    fn test_calculate_ledger_xp_tiers() {
        // Medium risk
        assert_eq!(XpManager::calculate_ledger_xp("capability.grant"), 25);
        assert_eq!(XpManager::calculate_ledger_xp("capability.revoke"), 25);
        assert_eq!(XpManager::calculate_ledger_xp("approval.granted"), 25);
        assert_eq!(XpManager::calculate_ledger_xp("approval.denied"), 25);

        // High risk
        assert_eq!(XpManager::calculate_ledger_xp("db.migration"), 50);
        assert_eq!(XpManager::calculate_ledger_xp("exec.shell"), 50);
        assert_eq!(XpManager::calculate_ledger_xp("worker.quarantined"), 50);

        // Low risk (everything else)
        assert_eq!(XpManager::calculate_ledger_xp("config.change"), 10);
        assert_eq!(XpManager::calculate_ledger_xp("safe_mode.enabled"), 10);
        assert_eq!(XpManager::calculate_ledger_xp("safe_mode.disabled"), 10);
    }

    #[test]
    fn test_xp_source_to_source_str() {
        let tc = XpSource::TaskCompletion {
            task_id: Uuid::new_v4(),
            skill_id: None,
        };
        assert_eq!(tc.to_source_str(), "task_completion");

        let ls = XpSource::LedgerSigning {
            ledger_event_id: 42,
        };
        assert_eq!(ls.to_source_str(), "ledger_signing");

        let su = XpSource::SkillUsage {
            skill_id: Uuid::new_v4(),
        };
        assert_eq!(su.to_source_str(), "skill_usage");

        assert_eq!(XpSource::QualityBonus.to_source_str(), "quality_bonus");
    }
}
