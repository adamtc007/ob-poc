//! Monitoring Service - CRUD operations for Ongoing Monitoring
//!
//! This module provides database operations for monitoring setup,
//! monitoring events, and scheduled reviews.

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDate, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value as JsonValue;
use sqlx::{FromRow, PgPool};
use tracing::info;
use uuid::Uuid;

/// Monitoring setup record
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MonitoringSetupRow {
    pub setup_id: Uuid,
    pub cbu_id: Uuid,
    pub monitoring_level: String,
    pub components: Option<JsonValue>,
    pub active: Option<bool>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
}

/// Fields for creating/updating monitoring setup
#[derive(Debug, Clone)]
pub struct MonitoringSetupFields {
    pub cbu_id: Uuid,
    pub monitoring_level: String,
    pub components: Option<JsonValue>,
}

/// Monitoring event record
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct MonitoringEventRow {
    pub event_id: Uuid,
    pub cbu_id: Uuid,
    pub event_type: String,
    pub description: Option<String>,
    pub severity: Option<String>,
    pub requires_review: Option<bool>,
    pub reviewed_by: Option<String>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub review_outcome: Option<String>,
    pub review_notes: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

/// Fields for creating a monitoring event
#[derive(Debug, Clone)]
pub struct NewMonitoringEventFields {
    pub cbu_id: Uuid,
    pub event_type: String,
    pub description: Option<String>,
    pub severity: Option<String>,
    pub requires_review: Option<bool>,
}

/// Scheduled review record
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct ScheduledReviewRow {
    pub review_id: Uuid,
    pub cbu_id: Uuid,
    pub review_type: String,
    pub due_date: NaiveDate,
    pub assigned_to: Option<String>,
    pub status: Option<String>,
    pub completed_by: Option<String>,
    pub completed_at: Option<DateTime<Utc>>,
    pub completion_notes: Option<String>,
    pub next_review_id: Option<Uuid>,
    pub created_at: Option<DateTime<Utc>>,
}

/// Fields for scheduling a review
#[derive(Debug, Clone)]
pub struct NewScheduledReviewFields {
    pub cbu_id: Uuid,
    pub review_type: String,
    pub due_date: NaiveDate,
    pub assigned_to: Option<String>,
}

/// Service for monitoring operations
#[derive(Clone, Debug)]
pub struct MonitoringService {
    pool: PgPool,
}

impl MonitoringService {
    /// Create a new monitoring service
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Get reference to the connection pool
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    // =========================================================================
    // Monitoring Setup
    // =========================================================================

    /// Setup monitoring for a CBU (upsert - one setup per CBU)
    pub async fn setup_monitoring(&self, fields: &MonitoringSetupFields) -> Result<Uuid> {
        let setup_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".monitoring_setup
                (setup_id, cbu_id, monitoring_level, components, active, created_at, updated_at)
            VALUES ($1, $2, $3, $4, TRUE, NOW(), NOW())
            ON CONFLICT (cbu_id)
            DO UPDATE SET
                monitoring_level = EXCLUDED.monitoring_level,
                components = EXCLUDED.components,
                updated_at = NOW()
            "#,
        )
        .bind(setup_id)
        .bind(fields.cbu_id)
        .bind(&fields.monitoring_level)
        .bind(&fields.components)
        .execute(&self.pool)
        .await
        .context("Failed to setup monitoring")?;

        info!(
            "Setup {} monitoring for CBU {}",
            fields.monitoring_level, fields.cbu_id
        );

        // Return existing setup_id if it existed
        let actual_id = sqlx::query_scalar::<_, Uuid>(
            r#"
            SELECT setup_id FROM "ob-poc".monitoring_setup WHERE cbu_id = $1
            "#,
        )
        .bind(fields.cbu_id)
        .fetch_one(&self.pool)
        .await
        .context("Failed to get monitoring setup ID")?;

        Ok(actual_id)
    }

    /// Get monitoring setup for a CBU
    pub async fn get_monitoring_setup(&self, cbu_id: Uuid) -> Result<Option<MonitoringSetupRow>> {
        let result = sqlx::query_as::<_, MonitoringSetupRow>(
            r#"
            SELECT setup_id, cbu_id, monitoring_level, components, active, created_at, updated_at
            FROM "ob-poc".monitoring_setup
            WHERE cbu_id = $1
            "#,
        )
        .bind(cbu_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get monitoring setup")?;

        Ok(result)
    }

    /// Deactivate monitoring for a CBU
    pub async fn deactivate_monitoring(&self, cbu_id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".monitoring_setup
            SET active = FALSE, updated_at = NOW()
            WHERE cbu_id = $1
            "#,
        )
        .bind(cbu_id)
        .execute(&self.pool)
        .await
        .context("Failed to deactivate monitoring")?;

        if result.rows_affected() > 0 {
            info!("Deactivated monitoring for CBU {}", cbu_id);
        }

        Ok(result.rows_affected() > 0)
    }

    // =========================================================================
    // Monitoring Events
    // =========================================================================

    /// Record a monitoring event
    pub async fn record_event(&self, fields: &NewMonitoringEventFields) -> Result<Uuid> {
        let event_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".monitoring_events
                (event_id, cbu_id, event_type, description, severity, requires_review, created_at)
            VALUES ($1, $2, $3, $4, $5, $6, NOW())
            "#,
        )
        .bind(event_id)
        .bind(fields.cbu_id)
        .bind(&fields.event_type)
        .bind(&fields.description)
        .bind(&fields.severity)
        .bind(fields.requires_review)
        .execute(&self.pool)
        .await
        .context("Failed to record monitoring event")?;

        info!(
            "Recorded {} event {} for CBU {}",
            fields.event_type, event_id, fields.cbu_id
        );

        Ok(event_id)
    }

    /// Get event by ID
    pub async fn get_event_by_id(&self, event_id: Uuid) -> Result<Option<MonitoringEventRow>> {
        let result = sqlx::query_as::<_, MonitoringEventRow>(
            r#"
            SELECT event_id, cbu_id, event_type, description, severity, requires_review,
                   reviewed_by, reviewed_at, review_outcome, review_notes, created_at
            FROM "ob-poc".monitoring_events
            WHERE event_id = $1
            "#,
        )
        .bind(event_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get event by ID")?;

        Ok(result)
    }

    /// List events for a CBU
    pub async fn list_events_for_cbu(&self, cbu_id: Uuid) -> Result<Vec<MonitoringEventRow>> {
        let results = sqlx::query_as::<_, MonitoringEventRow>(
            r#"
            SELECT event_id, cbu_id, event_type, description, severity, requires_review,
                   reviewed_by, reviewed_at, review_outcome, review_notes, created_at
            FROM "ob-poc".monitoring_events
            WHERE cbu_id = $1
            ORDER BY created_at DESC
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to list events for CBU")?;

        Ok(results)
    }

    /// List events requiring review
    pub async fn list_events_requiring_review(&self) -> Result<Vec<MonitoringEventRow>> {
        let results = sqlx::query_as::<_, MonitoringEventRow>(
            r#"
            SELECT event_id, cbu_id, event_type, description, severity, requires_review,
                   reviewed_by, reviewed_at, review_outcome, review_notes, created_at
            FROM "ob-poc".monitoring_events
            WHERE requires_review = TRUE AND reviewed_at IS NULL
            ORDER BY created_at ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to list events requiring review")?;

        Ok(results)
    }

    /// Review an event
    pub async fn review_event(
        &self,
        event_id: Uuid,
        reviewed_by: &str,
        outcome: &str,
        notes: Option<&str>,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".monitoring_events
            SET reviewed_by = $1,
                reviewed_at = NOW(),
                review_outcome = $2,
                review_notes = $3
            WHERE event_id = $4
            "#,
        )
        .bind(reviewed_by)
        .bind(outcome)
        .bind(notes)
        .bind(event_id)
        .execute(&self.pool)
        .await
        .context("Failed to review event")?;

        if result.rows_affected() > 0 {
            info!("Reviewed event {} with outcome '{}'", event_id, outcome);
        }

        Ok(result.rows_affected() > 0)
    }

    // =========================================================================
    // Scheduled Reviews
    // =========================================================================

    /// Schedule a review
    pub async fn schedule_review(&self, fields: &NewScheduledReviewFields) -> Result<Uuid> {
        let review_id = Uuid::new_v4();

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".scheduled_reviews
                (review_id, cbu_id, review_type, due_date, assigned_to, status, created_at)
            VALUES ($1, $2, $3, $4, $5, 'SCHEDULED', NOW())
            "#,
        )
        .bind(review_id)
        .bind(fields.cbu_id)
        .bind(&fields.review_type)
        .bind(fields.due_date)
        .bind(&fields.assigned_to)
        .execute(&self.pool)
        .await
        .context("Failed to schedule review")?;

        info!(
            "Scheduled {} review {} for CBU {} due {}",
            fields.review_type, review_id, fields.cbu_id, fields.due_date
        );

        Ok(review_id)
    }

    /// Get scheduled review by ID
    pub async fn get_review_by_id(&self, review_id: Uuid) -> Result<Option<ScheduledReviewRow>> {
        let result = sqlx::query_as::<_, ScheduledReviewRow>(
            r#"
            SELECT review_id, cbu_id, review_type, due_date, assigned_to, status,
                   completed_by, completed_at, completion_notes, next_review_id, created_at
            FROM "ob-poc".scheduled_reviews
            WHERE review_id = $1
            "#,
        )
        .bind(review_id)
        .fetch_optional(&self.pool)
        .await
        .context("Failed to get review by ID")?;

        Ok(result)
    }

    /// List scheduled reviews for a CBU
    pub async fn list_reviews_for_cbu(&self, cbu_id: Uuid) -> Result<Vec<ScheduledReviewRow>> {
        let results = sqlx::query_as::<_, ScheduledReviewRow>(
            r#"
            SELECT review_id, cbu_id, review_type, due_date, assigned_to, status,
                   completed_by, completed_at, completion_notes, next_review_id, created_at
            FROM "ob-poc".scheduled_reviews
            WHERE cbu_id = $1
            ORDER BY due_date ASC
            "#,
        )
        .bind(cbu_id)
        .fetch_all(&self.pool)
        .await
        .context("Failed to list reviews for CBU")?;

        Ok(results)
    }

    /// List due/overdue reviews
    pub async fn list_due_reviews(&self) -> Result<Vec<ScheduledReviewRow>> {
        let results = sqlx::query_as::<_, ScheduledReviewRow>(
            r#"
            SELECT review_id, cbu_id, review_type, due_date, assigned_to, status,
                   completed_by, completed_at, completion_notes, next_review_id, created_at
            FROM "ob-poc".scheduled_reviews
            WHERE status IN ('SCHEDULED', 'IN_PROGRESS') AND due_date <= CURRENT_DATE
            ORDER BY due_date ASC
            "#,
        )
        .fetch_all(&self.pool)
        .await
        .context("Failed to list due reviews")?;

        Ok(results)
    }

    /// Start a review (mark as in progress)
    pub async fn start_review(&self, review_id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".scheduled_reviews
            SET status = 'IN_PROGRESS'
            WHERE review_id = $1
            "#,
        )
        .bind(review_id)
        .execute(&self.pool)
        .await
        .context("Failed to start review")?;

        if result.rows_affected() > 0 {
            info!("Started review {}", review_id);
        }

        Ok(result.rows_affected() > 0)
    }

    /// Complete a review
    pub async fn complete_review(
        &self,
        review_id: Uuid,
        completed_by: &str,
        notes: Option<&str>,
        next_review_id: Option<Uuid>,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".scheduled_reviews
            SET status = 'COMPLETED',
                completed_by = $1,
                completed_at = NOW(),
                completion_notes = $2,
                next_review_id = $3
            WHERE review_id = $4
            "#,
        )
        .bind(completed_by)
        .bind(notes)
        .bind(next_review_id)
        .bind(review_id)
        .execute(&self.pool)
        .await
        .context("Failed to complete review")?;

        if result.rows_affected() > 0 {
            info!("Completed review {} by {}", review_id, completed_by);
        }

        Ok(result.rows_affected() > 0)
    }

    /// Mark review as overdue
    pub async fn mark_overdue(&self, review_id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            r#"
            UPDATE "ob-poc".scheduled_reviews
            SET status = 'OVERDUE'
            WHERE review_id = $1 AND status = 'SCHEDULED'
            "#,
        )
        .bind(review_id)
        .execute(&self.pool)
        .await
        .context("Failed to mark review as overdue")?;

        if result.rows_affected() > 0 {
            info!("Marked review {} as overdue", review_id);
        }

        Ok(result.rows_affected() > 0)
    }

    /// Delete review
    pub async fn delete_review(&self, review_id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            r#"
            DELETE FROM "ob-poc".scheduled_reviews
            WHERE review_id = $1
            "#,
        )
        .bind(review_id)
        .execute(&self.pool)
        .await
        .context("Failed to delete review")?;

        if result.rows_affected() > 0 {
            info!("Deleted review {}", review_id);
        }

        Ok(result.rows_affected() > 0)
    }

    /// Delete event
    pub async fn delete_event(&self, event_id: Uuid) -> Result<bool> {
        let result = sqlx::query(
            r#"
            DELETE FROM "ob-poc".monitoring_events
            WHERE event_id = $1
            "#,
        )
        .bind(event_id)
        .execute(&self.pool)
        .await
        .context("Failed to delete event")?;

        if result.rows_affected() > 0 {
            info!("Deleted event {}", event_id);
        }

        Ok(result.rows_affected() > 0)
    }
}
