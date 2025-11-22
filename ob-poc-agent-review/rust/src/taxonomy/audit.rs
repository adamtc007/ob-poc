//! Audit logging for taxonomy operations
//!
//! Tracks all taxonomy operations for compliance and debugging.

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde_json::Value;
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Debug, Clone)]
pub struct AuditEntry {
    pub operation: String,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub user: String,
    pub timestamp: DateTime<Utc>,
    pub before_state: Option<Value>,
    pub after_state: Option<Value>,
    pub metadata: Option<Value>,
    pub success: bool,
    pub error_message: Option<String>,
}

pub struct AuditLogger {
    pool: PgPool,
}

impl AuditLogger {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Log a taxonomy operation
    pub async fn log_operation(&self, entry: AuditEntry) -> Result<Uuid> {
        let audit_id = sqlx::query_scalar!(
            r#"
            INSERT INTO "ob-poc".taxonomy_audit_log
            (operation, entity_type, entity_id, user_id, before_state, after_state,
             metadata, success, error_message)
            VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9)
            RETURNING audit_id
            "#,
            entry.operation,
            entry.entity_type,
            entry.entity_id,
            entry.user,
            entry.before_state,
            entry.after_state,
            entry.metadata,
            entry.success,
            entry.error_message
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(audit_id)
    }

    /// Log a state transition for an onboarding request
    pub async fn log_state_transition(
        &self,
        request_id: Uuid,
        from_state: &str,
        to_state: &str,
        user: &str,
        metadata: Option<Value>,
    ) -> Result<()> {
        let entry = AuditEntry {
            operation: "state_transition".to_string(),
            entity_type: "onboarding_request".to_string(),
            entity_id: request_id,
            user: user.to_string(),
            timestamp: Utc::now(),
            before_state: Some(serde_json::json!({ "state": from_state })),
            after_state: Some(serde_json::json!({ "state": to_state })),
            metadata,
            success: true,
            error_message: None,
        };

        self.log_operation(entry).await?;
        Ok(())
    }

    /// Get audit trail for an entity
    pub async fn get_audit_trail(&self, entity_id: Uuid) -> Result<Vec<AuditRecord>> {
        let records = sqlx::query_as!(
            AuditRecord,
            r#"
            SELECT
                audit_id,
                operation,
                entity_type,
                entity_id,
                user_id,
                before_state,
                after_state,
                metadata,
                success,
                error_message,
                created_at
            FROM "ob-poc".taxonomy_audit_log
            WHERE entity_id = $1
            ORDER BY created_at DESC
            "#,
            entity_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(records)
    }

    /// Get recent audit entries
    pub async fn get_recent_entries(&self, limit: i64) -> Result<Vec<AuditRecord>> {
        let records = sqlx::query_as!(
            AuditRecord,
            r#"
            SELECT
                audit_id,
                operation,
                entity_type,
                entity_id,
                user_id,
                before_state,
                after_state,
                metadata,
                success,
                error_message,
                created_at
            FROM "ob-poc".taxonomy_audit_log
            ORDER BY created_at DESC
            LIMIT $1
            "#,
            limit
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(records)
    }
}

#[derive(Debug, Clone)]
pub struct AuditRecord {
    pub audit_id: Uuid,
    pub operation: String,
    pub entity_type: String,
    pub entity_id: Uuid,
    pub user_id: String,
    pub before_state: Option<Value>,
    pub after_state: Option<Value>,
    pub metadata: Option<Value>,
    pub success: bool,
    pub error_message: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_audit_entry_creation() {
        let entry = AuditEntry {
            operation: "create_product".to_string(),
            entity_type: "product".to_string(),
            entity_id: Uuid::new_v4(),
            user: "test_user".to_string(),
            timestamp: Utc::now(),
            before_state: None,
            after_state: Some(serde_json::json!({"name": "Test Product"})),
            metadata: None,
            success: true,
            error_message: None,
        };

        assert_eq!(entry.operation, "create_product");
        assert!(entry.success);
    }
}
