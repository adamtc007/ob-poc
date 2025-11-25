//! Investigation Service - CRUD operations for KYC Investigations
//!
//! This module provides database operations for KYC investigations and assignments.
//! An investigation wraps the entire KYC workflow for one CBU.

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{FromRow, PgPool};
use tracing::info;
use uuid::Uuid;

/// KYC Investigation record
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct InvestigationRow {
    pub investigation_id: Uuid,
    pub cbu_id: Option<Uuid>,
    pub investigation_type: String,
    pub risk_rating: Option<String>,
    pub regulatory_framework: Option<JsonValue>,
    pub ubo_threshold: Option<rust_decimal::Decimal>,
    pub investigation_depth: Option<i32>,
    pub status: Option<String>,
    pub deadline: Option<NaiveDate>,
    pub outcome: Option<String>,
    pub notes: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}

/// Fields for creating a new investigation
#[derive(Debug, Clone, Default)]
pub struct NewInvestigationFields {
    pub cbu_id: Option<Uuid>,
    pub investigation_type: String,
    pub risk_rating: Option<String>,
    pub regulatory_framework: Option<JsonValue>,
    pub ubo_threshold: Option<f64>,
    pub investigation_depth: Option<i32>,
    pub deadline: Option<NaiveDate>,
}

/// Investigation assignment record
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct InvestigationAssignmentRow {
    pub assignment_id: Uuid,
    pub investigation_id: Uuid,
    pub assignee: String,
    pub role: Option<String>,
    pub assigned_at: Option<DateTime<Utc>>,
}

/// Fields for creating an assignment
#[derive(Debug, Clone)]
pub struct NewAssignmentFields {
    pub investigation_id: Uuid,
    pub assignee: String,
    pub role: Option<String>,
}

/// Service for investigation operations
#[derive(Clone, Debug)]
pub struct InvestigationService {
    pool: PgPool,
}

impl InvestigationService {
    /// Create a new investigation service
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get reference to the connection pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Create a new investigation
    pub async fn create_investigation(&self, fields: &NewInvestigationFields) -> Result<Uuid> {
        let investigation_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".kyc_investigations
                (investigation_id, cbu_id, investigation_type, risk_rating,
                 regulatory_framework, ubo_threshold, investigation_depth, deadline,
                 status, created_at, updated_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, 'INITIATED', NOW(), NOW())
            "#,
        )
        .bind(investigation_id)
        .bind(fields.cbu_id)
        .bind(&fields.investigation_type)
        .bind(&fields.risk_rating)
        .bind(&fields.regulatory_framework)
        .bind(fields.ubo_threshold)
        .bind(fields.investigation_depth)
        .bind(fields.deadline)
        .execute(&self.pool)
        .await
        .context("Failed to create investigation")?;

        info!(
            "Created investigation {} type '{}'",
            investigation_id, fields.investigation_type
        );

        Ok(investigation_id)
    }

    /// Get investigation by ID
    pub async fn get_investigation_by_id(
        &self,
        investigation_id: Uuid,
    ) -> Result<Option<InvestigationRow>> {
        let result = sqlx::query_as::<_, InvestigationRow>(
            r#"
            SELECT investigation_id, cbu_id, investigation_type, risk_rating,
                   regulatory_framework, ubo_threshold, investigation_depth,
                   status, deadline, outcome, notes, created_at, updated_at, completed_at
            FROM "ob-poc".kyc_investigations
            WHERE investigation_id = $1
            "#,
        )
        .bind(investigation_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get investigation by ID")?;

        Ok(result)
    }

    /// List investigations for a CBU
    pub async fn list_investigations_for_cbu(&self, cbu_id: Uuid) -> Result<Vec<InvestigationRow>> {
        let results = sqlx::query_as::<_, InvestigationRow>(
            r#"
            SELECT investigation_id, cbu_id, investigation_type, risk_rating,
                   regulatory_framework, ubo_threshold, investigation_depth,
                   status, deadline, outcome, notes, created_at, updated_at, completed_at
            FROM "ob-poc".kyc_investigations
            WHERE cbu_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to list investigations for CBU")?;

        Ok(results)
    }

    /// Update investigation status
    pub async fn update_status(&self, investigation_id: Uuid, status: &str) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".kyc_investigations
            SET status = $1, updated_at = NOW()
            WHERE investigation_id = $2
            "#,
        )
        .bind(status)
        .bind(investigation_id)
        .execute(&self.pool)
        .await
        .context("Failed to update investigation status")?;

        if result.rows_affected() > 0 {
            info!(
                "Updated investigation {} status to '{}'",
                investigation_id, status
            );
        }

        Ok(result.rows_affected() > 0)
    }

    /// Complete investigation
    pub async fn complete_investigation(
        &self,
        investigation_id: Uuid,
        outcome: &str,
        notes: Option<&str>,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".kyc_investigations
            SET status = 'COMPLETE',
                outcome = $1,
                notes = COALESCE($2, notes),
                completed_at = NOW(),
                updated_at = NOW()
            WHERE investigation_id = $3
            "#,
        )
        .bind(outcome)
        .bind(notes)
        .bind(investigation_id)
        .execute(&self.pool)
        .await
        .context("Failed to complete investigation")?;

        if result.rows_affected() > 0 {
            info!(
                "Completed investigation {} with outcome '{}'",
                investigation_id, outcome
            );
        }

        Ok(result.rows_affected() > 0)
    }

    /// Assign analyst to investigation
    pub async fn assign(&self, fields: &NewAssignmentFields) -> Result<Uuid> {
        let assignment_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".investigation_assignments
                (assignment_id, investigation_id, assignee, role, assigned_at)
            VALUES ($1, $2, $3, $4, NOW())
            ON CONFLICT (investigation_id, assignee, role) DO NOTHING
            "#,
        )
        .bind(assignment_id)
        .bind(fields.investigation_id)
        .bind(&fields.assignee)
        .bind(&fields.role)
        .execute(&self.pool)
        .await
        .context("Failed to assign to investigation")?;

        info!(
            "Assigned {} to investigation {} as {:?}",
            fields.assignee, fields.investigation_id, fields.role
        );

        Ok(assignment_id)
    }

    /// List assignments for an investigation
    pub async fn list_assignments(
        &self,
        investigation_id: Uuid,
    ) -> Result<Vec<InvestigationAssignmentRow>> {
        let results = sqlx::query_as::<_, InvestigationAssignmentRow>(
            r#"
            SELECT assignment_id, investigation_id, assignee, role, assigned_at
            FROM "ob-poc".investigation_assignments
            WHERE investigation_id = $1
            ORDER BY assigned_at DESC
            "#,
        )
        .bind(investigation_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to list assignments")?;

        Ok(results)
    }

    /// Delete investigation
    pub async fn delete_investigation(&self, investigation_id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            r#"
            DELETE FROM "ob-poc".kyc_investigations
            WHERE investigation_id = $1
            "#,
        )
        .bind(investigation_id)
        .execute(&self.pool)
        .await
        .context("Failed to delete investigation")?;

        if result.rows_affected() > 0 {
            info!("Deleted investigation {}", investigation_id);
        }

        Ok(result.rows_affected() > 0)
    }
}
