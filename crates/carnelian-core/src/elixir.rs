//! Elixir management for CARNELIAN.
//!
//! This module provides the `ElixirManager` for creating, querying, and managing elixirs
//! (knowledge artifacts) and their lifecycle. It touches four tables:
//! - `elixirs`: Main elixir records with metadata, dataset, and quality metrics
//! - `elixir_versions`: Version history for each elixir
//! - `elixir_usage`: Usage tracking and effectiveness scoring
//! - `elixir_drafts`: Auto-generated draft proposals awaiting review
//!
//! Auto-draft threshold rule: When a skill reaches 100+ usages and has no pending draft
//! or active elixir, a draft is automatically created for review.

use carnelian_common::types::{
    ApproveDraftResponse, CreateElixirRequest, ElixirDetail, ElixirDraft, ElixirSearchResponse,
    ListElixirDraftsResponse, ListElixirsQuery, ListElixirsResponse, RejectDraftResponse,
};
use carnelian_common::{Error, Result};
use serde_json::Value as JsonValue;
use sqlx::{PgPool, Row};
use std::sync::Arc;
use uuid::Uuid;

use crate::xp::{XpManager, XpSource};

pub struct ElixirManager {
    pool: PgPool,
    xp_manager: Arc<XpManager>,
}

impl ElixirManager {
    pub fn new(pool: PgPool, xp_manager: Arc<XpManager>) -> Self {
        Self { pool, xp_manager }
    }

    /// Maps a database row from the `elixirs` table to `ElixirDetail`.
    fn row_to_elixir_detail(r: &sqlx::postgres::PgRow) -> ElixirDetail {
        ElixirDetail {
            elixir_id: r.get("elixir_id"),
            name: r.get("name"),
            description: r.get("description"),
            elixir_type: r.get("elixir_type"),
            icon: r.get("icon"),
            created_by: r.get("created_by"),
            skill_id: r.get("skill_id"),
            dataset: r.get("dataset"),
            size_bytes: r.get("size_bytes"),
            version: r.get("version"),
            quality_score: r.get("quality_score"),
            security_integrity_hash: r.get("security_integrity_hash"),
            active: r.get("active"),
            created_at: r.get("created_at"),
            updated_at: r.get("updated_at"),
        }
    }

    /// Creates a new elixir with an initial version record.
    pub async fn create_elixir(
        &self,
        req: CreateElixirRequest,
        created_by: Option<Uuid>,
    ) -> Result<ElixirDetail> {
        let mut tx = self.pool.begin().await.map_err(Error::Database)?;

        let icon = req.icon.unwrap_or_else(|| "🧪".to_string());

        let elixir_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO elixirs (
                name, description, elixir_type, icon, created_by, skill_id,
                dataset, size_bytes, version, quality_score, active
            )
            VALUES ($1, $2, $3, $4, $5, $6, $7, 0, 1, 0.0, true)
            RETURNING elixir_id
            "#,
        )
        .bind(&req.name)
        .bind(&req.description)
        .bind(&req.elixir_type)
        .bind(&icon)
        .bind(created_by)
        .bind(req.skill_id)
        .bind(&req.dataset)
        .fetch_one(&mut *tx)
        .await
        .map_err(Error::Database)?;

        sqlx::query(
            r#"
            INSERT INTO elixir_versions (
                elixir_id, version_number, dataset, created_by, change_description
            )
            VALUES ($1, 1, $2, $3, 'Initial version')
            "#,
        )
        .bind(elixir_id)
        .bind(&req.dataset)
        .bind(created_by)
        .execute(&mut *tx)
        .await
        .map_err(Error::Database)?;

        tx.commit().await.map_err(Error::Database)?;

        // Award XP for elixir creation
        if let Some(identity_id) = created_by {
            if let Err(e) = self.xp_manager.ensure_agent_xp(identity_id).await {
                tracing::warn!(
                    identity_id = %identity_id,
                    error = %e,
                    "Failed to ensure agent XP record"
                );
            }
            if let Err(e) = self
                .xp_manager
                .award_xp(identity_id, XpSource::ElixirCreated { elixir_id }, 50, None)
                .await
            {
                tracing::warn!(
                    identity_id = %identity_id,
                    elixir_id = %elixir_id,
                    error = %e,
                    "Failed to award XP for elixir creation"
                );
            }
        }

        let elixir = self
            .get_elixir(elixir_id)
            .await?
            .ok_or_else(|| Error::Database(sqlx::Error::RowNotFound))?;

        tracing::info!(elixir_id = %elixir_id, name = %req.name, "Elixir created");

        Ok(elixir)
    }

    /// Retrieves a single elixir by ID.
    pub async fn get_elixir(&self, elixir_id: Uuid) -> Result<Option<ElixirDetail>> {
        let row = sqlx::query("SELECT * FROM elixirs WHERE elixir_id = $1")
            .bind(elixir_id)
            .fetch_optional(&self.pool)
            .await
            .map_err(Error::Database)?;

        Ok(row.map(|r| Self::row_to_elixir_detail(&r)))
    }

    /// Lists elixirs with optional filtering and pagination.
    pub async fn list_elixirs(&self, query: ListElixirsQuery) -> Result<ListElixirsResponse> {
        let mut sql = "SELECT * FROM elixirs WHERE 1=1".to_string();
        let mut count_sql = "SELECT COUNT(*) FROM elixirs WHERE 1=1".to_string();
        let mut param_index = 1;

        if query.elixir_type.is_some() {
            sql.push_str(&format!(" AND elixir_type = ${}", param_index));
            count_sql.push_str(&format!(" AND elixir_type = ${}", param_index));
            param_index += 1;
        }

        if query.skill_id.is_some() {
            sql.push_str(&format!(" AND skill_id = ${}", param_index));
            count_sql.push_str(&format!(" AND skill_id = ${}", param_index));
            param_index += 1;
        }

        if query.active.is_some() {
            sql.push_str(&format!(" AND active = ${}", param_index));
            count_sql.push_str(&format!(" AND active = ${}", param_index));
            param_index += 1;
        }

        let mut count_query = sqlx::query_scalar(&count_sql);
        if let Some(ref elixir_type) = query.elixir_type {
            count_query = count_query.bind(elixir_type);
        }
        if let Some(skill_id) = query.skill_id {
            count_query = count_query.bind(skill_id);
        }
        if let Some(active) = query.active {
            count_query = count_query.bind(active);
        }

        let total: i64 = count_query
            .fetch_one(&self.pool)
            .await
            .map_err(Error::Database)?;

        let offset = (query.page.saturating_sub(1)) * query.page_size;
        sql.push_str(&format!(
            " ORDER BY created_at DESC LIMIT ${} OFFSET ${}",
            param_index,
            param_index + 1
        ));

        let mut main_query = sqlx::query(&sql);
        if let Some(ref elixir_type) = query.elixir_type {
            main_query = main_query.bind(elixir_type);
        }
        if let Some(skill_id) = query.skill_id {
            main_query = main_query.bind(skill_id);
        }
        if let Some(active) = query.active {
            main_query = main_query.bind(active);
        }
        main_query = main_query.bind(query.page_size as i64);
        main_query = main_query.bind(offset as i64);

        let rows = main_query
            .fetch_all(&self.pool)
            .await
            .map_err(Error::Database)?;

        let elixirs = rows.iter().map(Self::row_to_elixir_detail).collect();

        Ok(ListElixirsResponse {
            elixirs,
            page: query.page,
            page_size: query.page_size,
            total,
        })
    }

    /// Searches elixirs using pgvector cosine distance (placeholder with zero-vector).
    pub async fn search_elixirs(&self, query: String, limit: u32) -> Result<ElixirSearchResponse> {
        let zero_vector = format!("[{}]", "0,".repeat(1535) + "0");

        let sql = format!(
            r#"
            SELECT * FROM elixirs
            WHERE active = true
            ORDER BY
              CASE WHEN embedding IS NOT NULL
                   THEN embedding <=> '{}'::vector
                   ELSE NULL
              END ASC NULLS LAST,
              quality_score DESC
            LIMIT $1
            "#,
            zero_vector
        );

        let rows = sqlx::query(&sql)
            .bind(limit as i64)
            .fetch_all(&self.pool)
            .await
            .map_err(Error::Database)?;

        let results = rows
            .iter()
            .map(Self::row_to_elixir_detail)
            .collect::<Vec<_>>();
        let total = results.len() as i64;

        Ok(ElixirSearchResponse {
            results,
            query,
            total,
        })
    }

    /// Approves a draft and promotes it to a full elixir.
    pub async fn approve_draft(
        &self,
        draft_id: Uuid,
        reviewed_by: Option<Uuid>,
    ) -> Result<ApproveDraftResponse> {
        let draft_row =
            sqlx::query("SELECT * FROM elixir_drafts WHERE draft_id = $1 AND status = 'pending'")
                .bind(draft_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(Error::Database)?
                .ok_or_else(|| {
                    Error::Validation("Draft not found or already reviewed".to_string())
                })?;

        let skill_id: Uuid = draft_row.get("skill_id");
        let proposed_name: String = draft_row.get("proposed_name");
        let proposed_description: Option<String> = draft_row.get("proposed_description");
        let dataset: JsonValue = draft_row.get("dataset");

        let mut tx = self.pool.begin().await.map_err(Error::Database)?;

        sqlx::query(
            r#"
            UPDATE elixir_drafts
            SET status = 'approved', reviewed_by = $2, reviewed_at = NOW()
            WHERE draft_id = $1
            "#,
        )
        .bind(draft_id)
        .bind(reviewed_by)
        .execute(&mut *tx)
        .await
        .map_err(Error::Database)?;

        let elixir_id: Uuid = sqlx::query_scalar(
            r#"
            INSERT INTO elixirs (
                name, description, elixir_type, skill_id, dataset, created_by, active,
                size_bytes, version, quality_score, icon
            )
            VALUES ($1, $2, 'training_data', $3, $4, $5, true, 0, 1, 0.0, '🧪')
            RETURNING elixir_id
            "#,
        )
        .bind(&proposed_name)
        .bind(&proposed_description)
        .bind(skill_id)
        .bind(&dataset)
        .bind(reviewed_by)
        .fetch_one(&mut *tx)
        .await
        .map_err(Error::Database)?;

        sqlx::query(
            r#"
            INSERT INTO elixir_versions (
                elixir_id, version_number, dataset, created_by, change_description
            )
            VALUES ($1, 1, $2, $3, 'Approved from draft')
            "#,
        )
        .bind(elixir_id)
        .bind(&dataset)
        .bind(reviewed_by)
        .execute(&mut *tx)
        .await
        .map_err(Error::Database)?;

        tx.commit().await.map_err(Error::Database)?;

        // Award XP for draft approval
        if let Some(identity_id) = reviewed_by {
            if let Err(e) = self.xp_manager.ensure_agent_xp(identity_id).await {
                tracing::warn!(
                    identity_id = %identity_id,
                    error = %e,
                    "Failed to ensure agent XP record"
                );
            }
            if let Err(e) = self
                .xp_manager
                .award_xp(identity_id, XpSource::ElixirApproved { draft_id }, 25, None)
                .await
            {
                tracing::warn!(
                    identity_id = %identity_id,
                    draft_id = %draft_id,
                    error = %e,
                    "Failed to award XP for draft approval"
                );
            }
        }

        tracing::info!(draft_id = %draft_id, elixir_id = %elixir_id, "Draft approved");

        Ok(ApproveDraftResponse {
            draft_id,
            elixir_id,
            approved: true,
        })
    }

    /// Rejects a draft.
    pub async fn reject_draft(
        &self,
        draft_id: Uuid,
        reviewed_by: Option<Uuid>,
    ) -> Result<RejectDraftResponse> {
        let result = sqlx::query(
            r#"
            UPDATE elixir_drafts
            SET status = 'rejected', reviewed_by = $2, reviewed_at = NOW()
            WHERE draft_id = $1 AND status = 'pending'
            "#,
        )
        .bind(draft_id)
        .bind(reviewed_by)
        .execute(&self.pool)
        .await
        .map_err(Error::Database)?;

        if result.rows_affected() == 0 {
            return Err(Error::Validation(
                "Draft not found or already reviewed".to_string(),
            ));
        }

        tracing::info!(draft_id = %draft_id, "Draft rejected");

        Ok(RejectDraftResponse {
            draft_id,
            rejected: true,
        })
    }

    /// Checks if a skill meets the auto-draft threshold and creates a draft if eligible.
    pub async fn check_auto_draft_threshold(&self, skill_id: Uuid) -> Result<()> {
        let usage_count: Option<i64> =
            sqlx::query_scalar("SELECT usage_count FROM skill_metrics WHERE skill_id = $1")
                .bind(skill_id)
                .fetch_optional(&self.pool)
                .await
                .map_err(Error::Database)?;

        if usage_count.unwrap_or(0) < 100 {
            return Ok(());
        }

        let pending_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM elixir_drafts WHERE skill_id = $1 AND status = 'pending'",
        )
        .bind(skill_id)
        .fetch_one(&self.pool)
        .await
        .map_err(Error::Database)?;

        if pending_count > 0 {
            return Ok(());
        }

        let active_count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM elixirs WHERE skill_id = $1 AND active = true",
        )
        .bind(skill_id)
        .fetch_one(&self.pool)
        .await
        .map_err(Error::Database)?;

        if active_count > 0 {
            return Ok(());
        }

        sqlx::query(
            r#"
            INSERT INTO elixir_drafts (
                skill_id, proposed_name, dataset, auto_created_reason, status
            )
            VALUES ($1, $2, '{}'::jsonb, 'usage_count >= 100', 'pending')
            "#,
        )
        .bind(skill_id)
        .bind(format!("Auto-draft for {}", skill_id))
        .execute(&self.pool)
        .await
        .map_err(Error::Database)?;

        tracing::info!(skill_id = %skill_id, "Auto-draft created for skill");

        Ok(())
    }

    /// Scores the effectiveness of an elixir usage.
    pub async fn score_effectiveness(&self, usage_id: Uuid, score: f32) -> Result<()> {
        if !(0.0..=1.0).contains(&score) {
            return Err(Error::Validation(
                "Effectiveness score must be between 0.0 and 1.0".to_string(),
            ));
        }

        sqlx::query("UPDATE elixir_usage SET effectiveness_score = $2 WHERE usage_id = $1")
            .bind(usage_id)
            .bind(score)
            .execute(&self.pool)
            .await
            .map_err(Error::Database)?;

        Ok(())
    }
}
