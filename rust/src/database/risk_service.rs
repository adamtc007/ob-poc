//! Risk Service - CRUD operations for Risk Assessments and Flags
//!
//! This module provides database operations for risk assessments of CBUs
//! and entities, as well as risk flags (red flags, amber flags, notes).

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{FromRow, PgPool};
use tracing::info;
use uuid::Uuid;

/// Risk assessment record
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RiskAssessmentRow {
    pub assessment_id: Uuid,
    pub cbu_id: Option<Uuid>,
    pub entity_id: Option<Uuid>,
    pub investigation_id: Option<Uuid>,
    pub assessment_type: String,
    pub rating: Option<String>,
    pub factors: Option<JsonValue>,
    pub methodology: Option<String>,
    pub rationale: Option<String>,
    pub assessed_by: Option<String>,
    pub assessed_at: Option<DateTime<Utc>>,
}

/// Fields for creating a risk assessment
#[derive(Debug, Clone, Default)]
pub struct NewRiskAssessmentFields {
    pub cbu_id: Option<Uuid>,
    pub entity_id: Option<Uuid>,
    pub investigation_id: Option<Uuid>,
    pub assessment_type: String,
    pub methodology: Option<String>,
}

/// Fields for setting a risk rating
#[derive(Debug, Clone)]
pub struct RiskRatingFields {
    pub cbu_id: Option<Uuid>,
    pub entity_id: Option<Uuid>,
    pub investigation_id: Option<Uuid>,
    pub rating: String,
    pub factors: Option<JsonValue>,
    pub rationale: Option<String>,
    pub assessed_by: Option<String>,
}

/// Risk flag record
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct RiskFlagRow {
    pub flag_id: Uuid,
    pub cbu_id: Option<Uuid>,
    pub entity_id: Option<Uuid>,
    pub investigation_id: Option<Uuid>,
    pub flag_type: String,
    pub description: Option<String>,
    pub status: Option<String>,
    pub flagged_by: Option<String>,
    pub flagged_at: Option<DateTime<Utc>>,
    pub resolved_by: Option<String>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolution_notes: Option<String>,
}

/// Fields for creating a risk flag
#[derive(Debug, Clone)]
pub struct NewRiskFlagFields {
    pub cbu_id: Option<Uuid>,
    pub entity_id: Option<Uuid>,
    pub investigation_id: Option<Uuid>,
    pub flag_type: String,
    pub description: Option<String>,
    pub flagged_by: Option<String>,
}

/// Service for risk operations
#[derive(Clone, Debug)]
pub struct RiskService {
    pool: PgPool,
}

impl RiskService {
    /// Create a new risk service
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get reference to the connection pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Create a risk assessment for an entity
    pub async fn assess_entity(&self, fields: &NewRiskAssessmentFields) -> Result<Uuid> {
        let assessment_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".risk_assessments
                (assessment_id, entity_id, investigation_id, assessment_type,
                 methodology, assessed_at)
            VALUES ($1, $2, $3, 'ENTITY', $4, NOW())
            "#,
        )
        .bind(assessment_id)
        .bind(fields.entity_id)
        .bind(fields.investigation_id)
        .bind(&fields.methodology)
        .execute(&self.pool)
        .await
        .context("Failed to create entity risk assessment")?;

        info!(
            "Created entity risk assessment {} for entity {:?}",
            assessment_id, fields.entity_id
        );

        Ok(assessment_id)
    }

    /// Create a risk assessment for a CBU
    pub async fn assess_cbu(&self, fields: &NewRiskAssessmentFields) -> Result<Uuid> {
        let assessment_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".risk_assessments
                (assessment_id, cbu_id, investigation_id, assessment_type,
                 methodology, assessed_at)
            VALUES ($1, $2, $3, 'CBU', $4, NOW())
            "#,
        )
        .bind(assessment_id)
        .bind(fields.cbu_id)
        .bind(fields.investigation_id)
        .bind(&fields.methodology)
        .execute(&self.pool)
        .await
        .context("Failed to create CBU risk assessment")?;

        info!(
            "Created CBU risk assessment {} for CBU {:?}",
            assessment_id, fields.cbu_id
        );

        Ok(assessment_id)
    }

    /// Set risk rating (can be used to update an existing assessment or create new)
    pub async fn set_rating(&self, fields: &RiskRatingFields) -> Result<Uuid> {
        let assessment_id = Uuid::new_v4();
        let assessment_type = if fields.cbu_id.is_some() {
            "CBU"
        } else {
            "ENTITY"
        };

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".risk_assessments
                (assessment_id, cbu_id, entity_id, investigation_id, assessment_type,
                 rating, factors, rationale, assessed_by, assessed_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NOW())
            "#,
        )
        .bind(assessment_id)
        .bind(fields.cbu_id)
        .bind(fields.entity_id)
        .bind(fields.investigation_id)
        .bind(assessment_type)
        .bind(&fields.rating)
        .bind(&fields.factors)
        .bind(&fields.rationale)
        .bind(&fields.assessed_by)
        .execute(&self.pool)
        .await
        .context("Failed to set risk rating")?;

        info!(
            "Set risk rating '{}' for assessment {}",
            fields.rating, assessment_id
        );

        Ok(assessment_id)
    }

    /// Get risk assessment by ID
    pub async fn get_assessment_by_id(
        &self,
        assessment_id: Uuid,
    ) -> Result<Option<RiskAssessmentRow>> {
        let result = sqlx::query_as::<_, RiskAssessmentRow>(
            r#"
            SELECT assessment_id, cbu_id, entity_id, investigation_id,
                   assessment_type, rating, factors, methodology, rationale,
                   assessed_by, assessed_at
            FROM "ob-poc".risk_assessments
            WHERE assessment_id = $1
            "#,
        )
        .bind(assessment_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get risk assessment by ID")?;

        Ok(result)
    }

    /// List risk assessments for a CBU
    pub async fn list_assessments_for_cbu(&self, cbu_id: Uuid) -> Result<Vec<RiskAssessmentRow>> {
        let results = sqlx::query_as::<_, RiskAssessmentRow>(
            r#"
            SELECT assessment_id, cbu_id, entity_id, investigation_id,
                   assessment_type, rating, factors, methodology, rationale,
                   assessed_by, assessed_at
            FROM "ob-poc".risk_assessments
            WHERE cbu_id = $1
            ORDER BY assessed_at DESC
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to list risk assessments for CBU")?;

        Ok(results)
    }

    /// Get latest risk rating for a CBU
    pub async fn get_latest_rating_for_cbu(&self, cbu_id: Uuid) -> Result<Option<String>> {
        let result = sqlx::query_scalar::<_, String>(
            r#"
            SELECT rating
            FROM "ob-poc".risk_assessments
            WHERE cbu_id = $1 AND rating IS NOT NULL
            ORDER BY assessed_at DESC
            LIMIT 1
            "#,
        )
        .bind(cbu_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get latest risk rating for CBU")?;

        Ok(result)
    }

    /// Add a risk flag
    pub async fn add_flag(&self, fields: &NewRiskFlagFields) -> Result<Uuid> {
        let flag_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".risk_flags
                (flag_id, cbu_id, entity_id, investigation_id, flag_type,
                 description, status, flagged_by, flagged_at)
            VALUES ($1, $2, $3, $4, $5, $6, 'ACTIVE', $7, NOW())
            "#,
        )
        .bind(flag_id)
        .bind(fields.cbu_id)
        .bind(fields.entity_id)
        .bind(fields.investigation_id)
        .bind(&fields.flag_type)
        .bind(&fields.description)
        .bind(&fields.flagged_by)
        .execute(&self.pool)
        .await
        .context("Failed to add risk flag")?;

        info!(
            "Added {} flag {} for CBU {:?} / entity {:?}",
            fields.flag_type, flag_id, fields.cbu_id, fields.entity_id
        );

        Ok(flag_id)
    }

    /// Get risk flag by ID
    pub async fn get_flag_by_id(&self, flag_id: Uuid) -> Result<Option<RiskFlagRow>> {
        let result = sqlx::query_as::<_, RiskFlagRow>(
            r#"
            SELECT flag_id, cbu_id, entity_id, investigation_id, flag_type,
                   description, status, flagged_by, flagged_at,
                   resolved_by, resolved_at, resolution_notes
            FROM "ob-poc".risk_flags
            WHERE flag_id = $1
            "#,
        )
        .bind(flag_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get risk flag by ID")?;

        Ok(result)
    }

    /// List active flags for a CBU
    pub async fn list_active_flags_for_cbu(&self, cbu_id: Uuid) -> Result<Vec<RiskFlagRow>> {
        let results = sqlx::query_as::<_, RiskFlagRow>(
            r#"
            SELECT flag_id, cbu_id, entity_id, investigation_id, flag_type,
                   description, status, flagged_by, flagged_at,
                   resolved_by, resolved_at, resolution_notes
            FROM "ob-poc".risk_flags
            WHERE cbu_id = $1 AND status = 'ACTIVE'
            ORDER BY flagged_at DESC
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to list active flags for CBU")?;

        Ok(results)
    }

    /// Resolve a risk flag
    pub async fn resolve_flag(
        &self,
        flag_id: Uuid,
        resolved_by: Option<&str>,
        resolution_notes: Option<&str>,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".risk_flags
            SET status = 'RESOLVED',
                resolved_by = $1,
                resolved_at = NOW(),
                resolution_notes = $2
            WHERE flag_id = $3
            "#,
        )
        .bind(resolved_by)
        .bind(resolution_notes)
        .bind(flag_id)
        .execute(&self.pool)
        .await
        .context("Failed to resolve risk flag")?;

        if result.rows_affected() > 0 {
            info!("Resolved risk flag {}", flag_id);
        }

        Ok(result.rows_affected() > 0)
    }

    /// Delete risk assessment
    pub async fn delete_assessment(&self, assessment_id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            r#"
            DELETE FROM "ob-poc".risk_assessments
            WHERE assessment_id = $1
            "#,
        )
        .bind(assessment_id)
        .execute(&self.pool)
        .await
        .context("Failed to delete risk assessment")?;

        if result.rows_affected() > 0 {
            info!("Deleted risk assessment {}", assessment_id);
        }

        Ok(result.rows_affected() > 0)
    }

    /// Delete risk flag
    pub async fn delete_flag(&self, flag_id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            r#"
            DELETE FROM "ob-poc".risk_flags
            WHERE flag_id = $1
            "#,
        )
        .bind(flag_id)
        .execute(&self.pool)
        .await
        .context("Failed to delete risk flag")?;

        if result.rows_affected() > 0 {
            info!("Deleted risk flag {}", flag_id);
        }

        Ok(result.rows_affected() > 0)
    }
}
