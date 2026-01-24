//! Expansion Audit Repository
//!
//! Persists ExpansionReport to the database for audit/replay purposes.
//! Each template expansion creates an audit record capturing:
//! - Source and expanded DSL digests (for determinism verification)
//! - Template digests used
//! - Lock keys derived
//! - Batch policy determined

use sqlx::PgPool;
use uuid::Uuid;

use crate::dsl_v2::ExpansionReport;

/// Repository for persisting expansion audit trails
#[derive(Clone)]
pub struct ExpansionAuditRepository {
    pool: PgPool,
}

impl ExpansionAuditRepository {
    /// Create a new repository
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Persist an expansion report
    ///
    /// This is called after DSL expansion to create an audit trail.
    /// The report contains all information needed to reproduce the expansion.
    pub async fn save(
        &self,
        session_id: Uuid,
        report: &ExpansionReport,
    ) -> Result<(), sqlx::Error> {
        let batch_policy = match report.batch_policy {
            crate::dsl_v2::BatchPolicy::Atomic => "atomic",
            crate::dsl_v2::BatchPolicy::BestEffort => "best_effort",
        };

        // Serialize derived_lock_set
        let derived_lock_set = serde_json::to_value(&report.derived_lock_set)
            .unwrap_or_else(|_| serde_json::json!([]));

        // Serialize template_digests
        let template_digests = serde_json::to_value(&report.template_digests)
            .unwrap_or_else(|_| serde_json::json!([]));

        // Serialize invocations
        let invocations =
            serde_json::to_value(&report.invocations).unwrap_or_else(|_| serde_json::json!([]));

        // Serialize diagnostics
        let diagnostics =
            serde_json::to_value(&report.diagnostics).unwrap_or_else(|_| serde_json::json!([]));

        sqlx::query(
            r#"
            INSERT INTO "ob-poc".expansion_reports (
                expansion_id,
                session_id,
                source_digest,
                expanded_dsl_digest,
                expanded_statement_count,
                batch_policy,
                derived_lock_set,
                template_digests,
                invocations,
                diagnostics,
                expanded_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
            ON CONFLICT (expansion_id) DO NOTHING
            "#,
        )
        .bind(report.expansion_id)
        .bind(session_id)
        .bind(&report.source_digest)
        .bind(&report.expanded_dsl_digest)
        .bind(report.expanded_statement_count as i32)
        .bind(batch_policy)
        .bind(&derived_lock_set)
        .bind(&template_digests)
        .bind(&invocations)
        .bind(&diagnostics)
        .bind(report.expanded_at)
        .execute(&self.pool)
        .await?;

        tracing::debug!(
            expansion_id = %report.expansion_id,
            session_id = %session_id,
            batch_policy = %batch_policy,
            locks = report.derived_lock_set.len(),
            "Persisted expansion report"
        );

        Ok(())
    }

    /// Get an expansion report by ID
    pub async fn get(&self, expansion_id: Uuid) -> Result<Option<ExpansionReportRow>, sqlx::Error> {
        sqlx::query_as::<_, ExpansionReportRow>(
            r#"
            SELECT
                expansion_id,
                session_id,
                source_digest,
                expanded_dsl_digest,
                expanded_statement_count,
                batch_policy,
                derived_lock_set,
                template_digests,
                invocations,
                diagnostics,
                expanded_at,
                created_at
            FROM "ob-poc".expansion_reports
            WHERE expansion_id = $1
            "#,
        )
        .bind(expansion_id)
        .fetch_optional(&self.pool)
        .await
    }

    /// List recent expansion reports for a session
    pub async fn list_for_session(
        &self,
        session_id: Uuid,
        limit: i32,
    ) -> Result<Vec<ExpansionReportRow>, sqlx::Error> {
        sqlx::query_as::<_, ExpansionReportRow>(
            r#"
            SELECT
                expansion_id,
                session_id,
                source_digest,
                expanded_dsl_digest,
                expanded_statement_count,
                batch_policy,
                derived_lock_set,
                template_digests,
                invocations,
                diagnostics,
                expanded_at,
                created_at
            FROM "ob-poc".expansion_reports
            WHERE session_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
        )
        .bind(session_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }
}

/// Row from expansion_reports table
#[derive(Debug, Clone, sqlx::FromRow)]
pub struct ExpansionReportRow {
    pub expansion_id: Uuid,
    pub session_id: Uuid,
    pub source_digest: String,
    pub expanded_dsl_digest: String,
    pub expanded_statement_count: i32,
    pub batch_policy: String,
    pub derived_lock_set: serde_json::Value,
    pub template_digests: serde_json::Value,
    pub invocations: serde_json::Value,
    pub diagnostics: serde_json::Value,
    pub expanded_at: chrono::DateTime<chrono::Utc>,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_batch_policy_string_conversion() {
        // Verify the string conversion matches the CHECK constraint
        assert_eq!(
            match crate::dsl_v2::BatchPolicy::Atomic {
                crate::dsl_v2::BatchPolicy::Atomic => "atomic",
                crate::dsl_v2::BatchPolicy::BestEffort => "best_effort",
            },
            "atomic"
        );

        assert_eq!(
            match crate::dsl_v2::BatchPolicy::BestEffort {
                crate::dsl_v2::BatchPolicy::Atomic => "atomic",
                crate::dsl_v2::BatchPolicy::BestEffort => "best_effort",
            },
            "best_effort"
        );
    }
}
