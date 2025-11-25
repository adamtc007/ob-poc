//! Decision Service - CRUD operations for KYC Decisions and Conditions
//!
//! This module provides database operations for onboarding decisions
//! and associated conditions (for conditional acceptance).

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use sqlx::{FromRow, PgPool};
use tracing::info;
use uuid::Uuid;

/// KYC Decision record
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DecisionRow {
    pub decision_id: Uuid,
    pub cbu_id: Uuid,
    pub investigation_id: Option<Uuid>,
    pub decision: String,
    pub decision_authority: Option<String>,
    pub rationale: Option<String>,
    pub decided_by: Option<String>,
    pub decided_at: Option<DateTime<Utc>>,
    pub effective_date: Option<NaiveDate>,
    pub review_date: Option<NaiveDate>,
}

/// Fields for creating a decision
#[derive(Debug, Clone)]
pub struct NewDecisionFields {
    pub cbu_id: Uuid,
    pub investigation_id: Option<Uuid>,
    pub decision: String,
    pub decision_authority: Option<String>,
    pub rationale: Option<String>,
    pub decided_by: Option<String>,
}

/// Decision condition record
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct DecisionConditionRow {
    pub condition_id: Uuid,
    pub decision_id: Uuid,
    pub condition_type: String,
    pub description: Option<String>,
    pub frequency: Option<String>,
    pub due_date: Option<NaiveDate>,
    pub threshold: Option<rust_decimal::Decimal>,
    pub currency: Option<String>,
    pub assigned_to: Option<String>,
    pub status: Option<String>,
    pub satisfied_by: Option<String>,
    pub satisfied_at: Option<DateTime<Utc>>,
    pub satisfaction_evidence: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

/// Fields for creating a condition
#[derive(Debug, Clone)]
pub struct NewConditionFields {
    pub decision_id: Uuid,
    pub condition_type: String,
    pub description: Option<String>,
    pub frequency: Option<String>,
    pub due_date: Option<NaiveDate>,
    pub threshold: Option<f64>,
    pub currency: Option<String>,
    pub assigned_to: Option<String>,
}

/// Fields for satisfying a condition
#[derive(Debug, Clone)]
pub struct SatisfyConditionFields {
    pub condition_id: Uuid,
    pub satisfied_by: Option<String>,
    pub satisfaction_evidence: Option<String>,
}

/// Service for decision operations
#[derive(Clone, Debug)]
pub struct DecisionService {
    pool: PgPool,
}

impl DecisionService {
    /// Create a new decision service
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get reference to the connection pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Record a new decision
    pub async fn record_decision(&self, fields: &NewDecisionFields) -> Result<Uuid> {
        let decision_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".kyc_decisions
                (decision_id, cbu_id, investigation_id, decision, decision_authority,
                 rationale, decided_by, decided_at, effective_date)
            VALUES ($1, $2, $3, $4, $5, $6, $7, NOW(), CURRENT_DATE)
            "#,
        )
        .bind(decision_id)
        .bind(fields.cbu_id)
        .bind(fields.investigation_id)
        .bind(&fields.decision)
        .bind(&fields.decision_authority)
        .bind(&fields.rationale)
        .bind(&fields.decided_by)
        .execute(&self.pool)
        .await
        .context("Failed to record decision")?;

        info!(
            "Recorded decision {} '{}' for CBU {}",
            decision_id, fields.decision, fields.cbu_id
        );

        Ok(decision_id)
    }

    /// Get decision by ID
    pub async fn get_decision_by_id(&self, decision_id: Uuid) -> Result<Option<DecisionRow>> {
        let result = sqlx::query_as::<_, DecisionRow>(
            r#"
            SELECT decision_id, cbu_id, investigation_id, decision, decision_authority,
                   rationale, decided_by, decided_at, effective_date, review_date
            FROM "ob-poc".kyc_decisions
            WHERE decision_id = $1
            "#,
        )
        .bind(decision_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get decision by ID")?;

        Ok(result)
    }

    /// List decisions for a CBU
    pub async fn list_decisions_for_cbu(&self, cbu_id: Uuid) -> Result<Vec<DecisionRow>> {
        let results = sqlx::query_as::<_, DecisionRow>(
            r#"
            SELECT decision_id, cbu_id, investigation_id, decision, decision_authority,
                   rationale, decided_by, decided_at, effective_date, review_date
            FROM "ob-poc".kyc_decisions
            WHERE cbu_id = $1
            ORDER BY decided_at DESC
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to list decisions for CBU")?;

        Ok(results)
    }

    /// Get latest decision for a CBU
    pub async fn get_latest_decision_for_cbu(&self, cbu_id: Uuid) -> Result<Option<DecisionRow>> {
        let result = sqlx::query_as::<_, DecisionRow>(
            r#"
            SELECT decision_id, cbu_id, investigation_id, decision, decision_authority,
                   rationale, decided_by, decided_at, effective_date, review_date
            FROM "ob-poc".kyc_decisions
            WHERE cbu_id = $1
            ORDER BY decided_at DESC
            LIMIT 1
            "#,
        )
        .bind(cbu_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get latest decision for CBU")?;

        Ok(result)
    }

    /// Add a condition to a decision
    pub async fn add_condition(&self, fields: &NewConditionFields) -> Result<Uuid> {
        let condition_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".decision_conditions
                (condition_id, decision_id, condition_type, description, frequency,
                 due_date, threshold, currency, assigned_to, status, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'PENDING', NOW())
            "#,
        )
        .bind(condition_id)
        .bind(fields.decision_id)
        .bind(&fields.condition_type)
        .bind(&fields.description)
        .bind(&fields.frequency)
        .bind(fields.due_date)
        .bind(fields.threshold)
        .bind(&fields.currency)
        .bind(&fields.assigned_to)
        .execute(&self.pool)
        .await
        .context("Failed to add condition")?;

        info!(
            "Added condition {} '{}' to decision {}",
            condition_id, fields.condition_type, fields.decision_id
        );

        Ok(condition_id)
    }

    /// Get condition by ID
    pub async fn get_condition_by_id(
        &self,
        condition_id: Uuid,
    ) -> Result<Option<DecisionConditionRow>> {
        let result = sqlx::query_as::<_, DecisionConditionRow>(
            r#"
            SELECT condition_id, decision_id, condition_type, description, frequency,
                   due_date, threshold, currency, assigned_to, status,
                   satisfied_by, satisfied_at, satisfaction_evidence, created_at
            FROM "ob-poc".decision_conditions
            WHERE condition_id = $1
            "#,
        )
        .bind(condition_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get condition by ID")?;

        Ok(result)
    }

    /// List conditions for a decision
    pub async fn list_conditions_for_decision(
        &self,
        decision_id: Uuid,
    ) -> Result<Vec<DecisionConditionRow>> {
        let results = sqlx::query_as::<_, DecisionConditionRow>(
            r#"
            SELECT condition_id, decision_id, condition_type, description, frequency,
                   due_date, threshold, currency, assigned_to, status,
                   satisfied_by, satisfied_at, satisfaction_evidence, created_at
            FROM "ob-poc".decision_conditions
            WHERE decision_id = $1
            ORDER BY created_at ASC
            "#,
        )
        .bind(decision_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to list conditions for decision")?;

        Ok(results)
    }

    /// List pending conditions for a CBU
    pub async fn list_pending_conditions_for_cbu(
        &self,
        cbu_id: Uuid,
    ) -> Result<Vec<DecisionConditionRow>> {
        let results = sqlx::query_as::<_, DecisionConditionRow>(
            r#"
            SELECT c.condition_id, c.decision_id, c.condition_type, c.description, c.frequency,
                   c.due_date, c.threshold, c.currency, c.assigned_to, c.status,
                   c.satisfied_by, c.satisfied_at, c.satisfaction_evidence, c.created_at
            FROM "ob-poc".decision_conditions c
            JOIN "ob-poc".kyc_decisions d ON c.decision_id = d.decision_id
            WHERE d.cbu_id = $1 AND c.status = 'PENDING'
            ORDER BY c.due_date ASC
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to list pending conditions for CBU")?;

        Ok(results)
    }

    /// Satisfy a condition
    pub async fn satisfy_condition(&self, fields: &SatisfyConditionFields) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".decision_conditions
            SET status = 'SATISFIED',
                satisfied_by = $1,
                satisfied_at = NOW(),
                satisfaction_evidence = $2
            WHERE condition_id = $3
            "#,
        )
        .bind(&fields.satisfied_by)
        .bind(&fields.satisfaction_evidence)
        .bind(fields.condition_id)
        .execute(&self.pool)
        .await
        .context("Failed to satisfy condition")?;

        if result.rows_affected() > 0 {
            info!("Satisfied condition {}", fields.condition_id);
        }

        Ok(result.rows_affected() > 0)
    }

    /// Waive a condition
    pub async fn waive_condition(
        &self,
        condition_id: Uuid,
        waived_by: Option<&str>,
        reason: Option<&str>,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".decision_conditions
            SET status = 'WAIVED',
                satisfied_by = $1,
                satisfied_at = NOW(),
                satisfaction_evidence = $2
            WHERE condition_id = $3
            "#,
        )
        .bind(waived_by)
        .bind(reason)
        .bind(condition_id)
        .execute(&self.pool)
        .await
        .context("Failed to waive condition")?;

        if result.rows_affected() > 0 {
            info!("Waived condition {}", condition_id);
        }

        Ok(result.rows_affected() > 0)
    }

    /// Set review date for a decision
    pub async fn set_review_date(&self, decision_id: Uuid, review_date: NaiveDate) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".kyc_decisions
            SET review_date = $1
            WHERE decision_id = $2
            "#,
        )
        .bind(review_date)
        .bind(decision_id)
        .execute(&self.pool)
        .await
        .context("Failed to set review date")?;

        if result.rows_affected() > 0 {
            info!(
                "Set review date {} for decision {}",
                review_date, decision_id
            );
        }

        Ok(result.rows_affected() > 0)
    }

    /// Delete decision
    pub async fn delete_decision(&self, decision_id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            r#"
            DELETE FROM "ob-poc".kyc_decisions
            WHERE decision_id = $1
            "#,
        )
        .bind(decision_id)
        .execute(&self.pool)
        .await
        .context("Failed to delete decision")?;

        if result.rows_affected() > 0 {
            info!("Deleted decision {}", decision_id);
        }

        Ok(result.rows_affected() > 0)
    }

    /// Delete condition
    pub async fn delete_condition(&self, condition_id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            r#"
            DELETE FROM "ob-poc".decision_conditions
            WHERE condition_id = $1
            "#,
        )
        .bind(condition_id)
        .execute(&self.pool)
        .await
        .context("Failed to delete condition")?;

        if result.rows_affected() > 0 {
            info!("Deleted condition {}", condition_id);
        }

        Ok(result.rows_affected() > 0)
    }
}
