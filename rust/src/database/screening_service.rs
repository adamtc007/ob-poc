//! Screening Service - CRUD operations for PEP, Sanctions, and Adverse Media screening
//!
//! This module provides database operations for screening records including
//! PEP checks, sanctions screening, and adverse media searches.

use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{FromRow, PgPool};
use tracing::info;
use uuid::Uuid;

/// Screening record
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ScreeningRow {
    pub screening_id: Uuid,
    pub investigation_id: Option<Uuid>,
    pub entity_id: Uuid,
    pub screening_type: String,
    pub databases: Option<JsonValue>,
    pub lists: Option<JsonValue>,
    pub include_rca: Option<bool>,
    pub search_depth: Option<String>,
    pub languages: Option<JsonValue>,
    pub status: Option<String>,
    pub result: Option<String>,
    pub match_details: Option<JsonValue>,
    pub resolution: Option<String>,
    pub resolution_rationale: Option<String>,
    pub screened_at: Option<DateTime<Utc>>,
    pub reviewed_by: Option<String>,
    pub resolved_by: Option<String>,
    pub resolved_at: Option<DateTime<Utc>>,
}

/// Fields for creating a PEP screening
#[derive(Debug, Clone, Default)]
pub struct NewPepScreeningFields {
    pub investigation_id: Option<Uuid>,
    pub entity_id: Uuid,
    pub databases: Option<JsonValue>,
    pub include_rca: Option<bool>,
}

/// Fields for creating a sanctions screening
#[derive(Debug, Clone, Default)]
pub struct NewSanctionsScreeningFields {
    pub investigation_id: Option<Uuid>,
    pub entity_id: Uuid,
    pub lists: Option<JsonValue>,
}

/// Fields for creating an adverse media screening
#[derive(Debug, Clone, Default)]
pub struct NewAdverseMediaScreeningFields {
    pub investigation_id: Option<Uuid>,
    pub entity_id: Uuid,
    pub search_depth: Option<String>,
    pub languages: Option<JsonValue>,
}

/// Fields for recording screening result
#[derive(Debug, Clone)]
pub struct ScreeningResultFields {
    pub screening_id: Uuid,
    pub result: String,
    pub match_details: Option<JsonValue>,
    pub reviewed_by: Option<String>,
}

/// Fields for resolving a screening
#[derive(Debug, Clone)]
pub struct ScreeningResolutionFields {
    pub screening_id: Uuid,
    pub resolution: String,
    pub rationale: Option<String>,
    pub resolved_by: Option<String>,
}

/// Service for screening operations
#[derive(Clone, Debug)]
pub struct ScreeningService {
    pool: PgPool,
}

impl ScreeningService {
    /// Create a new screening service
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get reference to the connection pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Create a PEP screening
    pub async fn create_pep_screening(&self, fields: &NewPepScreeningFields) -> Result<Uuid> {
        let screening_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".screenings
                (screening_id, investigation_id, entity_id, screening_type,
                 databases, include_rca, status, screened_at)
            VALUES ($1, $2, $3, 'PEP', $4, $5, 'PENDING', NOW())
            "#,
        )
        .bind(screening_id)
        .bind(fields.investigation_id)
        .bind(fields.entity_id)
        .bind(&fields.databases)
        .bind(fields.include_rca)
        .execute(&self.pool)
        .await
        .context("Failed to create PEP screening")?;

        info!(
            "Created PEP screening {} for entity {}",
            screening_id, fields.entity_id
        );

        Ok(screening_id)
    }

    /// Create a sanctions screening
    pub async fn create_sanctions_screening(
        &self,
        fields: &NewSanctionsScreeningFields,
    ) -> Result<Uuid> {
        let screening_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".screenings
                (screening_id, investigation_id, entity_id, screening_type,
                 lists, status, screened_at)
            VALUES ($1, $2, $3, 'SANCTIONS', $4, 'PENDING', NOW())
            "#,
        )
        .bind(screening_id)
        .bind(fields.investigation_id)
        .bind(fields.entity_id)
        .bind(&fields.lists)
        .execute(&self.pool)
        .await
        .context("Failed to create sanctions screening")?;

        info!(
            "Created sanctions screening {} for entity {}",
            screening_id, fields.entity_id
        );

        Ok(screening_id)
    }

    /// Create an adverse media screening
    pub async fn create_adverse_media_screening(
        &self,
        fields: &NewAdverseMediaScreeningFields,
    ) -> Result<Uuid> {
        let screening_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".screenings
                (screening_id, investigation_id, entity_id, screening_type,
                 search_depth, languages, status, screened_at)
            VALUES ($1, $2, $3, 'ADVERSE_MEDIA', $4, $5, 'PENDING', NOW())
            "#,
        )
        .bind(screening_id)
        .bind(fields.investigation_id)
        .bind(fields.entity_id)
        .bind(&fields.search_depth)
        .bind(&fields.languages)
        .execute(&self.pool)
        .await
        .context("Failed to create adverse media screening")?;

        info!(
            "Created adverse media screening {} for entity {}",
            screening_id, fields.entity_id
        );

        Ok(screening_id)
    }

    /// Get screening by ID
    pub async fn get_screening_by_id(&self, screening_id: Uuid) -> Result<Option<ScreeningRow>> {
        let result = sqlx::query_as::<_, ScreeningRow>(
            r#"
            SELECT screening_id, investigation_id, entity_id, screening_type,
                   databases, lists, include_rca, search_depth, languages,
                   status, result, match_details, resolution, resolution_rationale,
                   screened_at, reviewed_by, resolved_by, resolved_at
            FROM "ob-poc".screenings
            WHERE screening_id = $1
            "#,
        )
        .bind(screening_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get screening by ID")?;

        Ok(result)
    }

    /// List screenings for an entity
    pub async fn list_screenings_for_entity(&self, entity_id: Uuid) -> Result<Vec<ScreeningRow>> {
        let results = sqlx::query_as::<_, ScreeningRow>(
            r#"
            SELECT screening_id, investigation_id, entity_id, screening_type,
                   databases, lists, include_rca, search_depth, languages,
                   status, result, match_details, resolution, resolution_rationale,
                   screened_at, reviewed_by, resolved_by, resolved_at
            FROM "ob-poc".screenings
            WHERE entity_id = $1
            ORDER BY screened_at DESC
            "#,
        )
        .bind(entity_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to list screenings for entity")?;

        Ok(results)
    }

    /// List screenings for an investigation
    pub async fn list_screenings_for_investigation(
        &self,
        investigation_id: Uuid,
    ) -> Result<Vec<ScreeningRow>> {
        let results = sqlx::query_as::<_, ScreeningRow>(
            r#"
            SELECT screening_id, investigation_id, entity_id, screening_type,
                   databases, lists, include_rca, search_depth, languages,
                   status, result, match_details, resolution, resolution_rationale,
                   screened_at, reviewed_by, resolved_by, resolved_at
            FROM "ob-poc".screenings
            WHERE investigation_id = $1
            ORDER BY screened_at DESC
            "#,
        )
        .bind(investigation_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to list screenings for investigation")?;

        Ok(results)
    }

    /// Record screening result
    pub async fn record_result(&self, fields: &ScreeningResultFields) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".screenings
            SET result = $1,
                match_details = $2,
                reviewed_by = $3,
                status = CASE
                    WHEN $1 IN ('NO_MATCH') THEN 'COMPLETED'
                    ELSE 'REVIEW_REQUIRED'
                END
            WHERE screening_id = $4
            "#,
        )
        .bind(&fields.result)
        .bind(&fields.match_details)
        .bind(&fields.reviewed_by)
        .bind(fields.screening_id)
        .execute(&self.pool)
        .await
        .context("Failed to record screening result")?;

        if result.rows_affected() > 0 {
            info!(
                "Recorded result '{}' for screening {}",
                fields.result, fields.screening_id
            );
        }

        Ok(result.rows_affected() > 0)
    }

    /// Resolve screening (mark as false positive, true hit, etc.)
    pub async fn resolve(&self, fields: &ScreeningResolutionFields) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".screenings
            SET resolution = $1,
                resolution_rationale = $2,
                resolved_by = $3,
                resolved_at = NOW(),
                status = 'COMPLETED'
            WHERE screening_id = $4
            "#,
        )
        .bind(&fields.resolution)
        .bind(&fields.rationale)
        .bind(&fields.resolved_by)
        .bind(fields.screening_id)
        .execute(&self.pool)
        .await
        .context("Failed to resolve screening")?;

        if result.rows_affected() > 0 {
            info!(
                "Resolved screening {} as '{}'",
                fields.screening_id, fields.resolution
            );
        }

        Ok(result.rows_affected() > 0)
    }

    /// Delete screening
    pub async fn delete_screening(&self, screening_id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            r#"
            DELETE FROM "ob-poc".screenings
            WHERE screening_id = $1
            "#,
        )
        .bind(screening_id)
        .execute(&self.pool)
        .await
        .context("Failed to delete screening")?;

        if result.rows_affected() > 0 {
            info!("Deleted screening {}", screening_id);
        }

        Ok(result.rows_affected() > 0)
    }
}
