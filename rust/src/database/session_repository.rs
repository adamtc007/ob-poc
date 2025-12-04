//! Session persistence repository
//!
//! Provides database-backed session state management for incremental DSL execution.
//! Sessions are persisted to dsl_sessions, snapshots to dsl_snapshots, events to dsl_session_events.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::collections::HashMap;
use uuid::Uuid;

// ============================================================================
// Types
// ============================================================================

/// Session status enum matching DB constraint
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, sqlx::Type)]
#[sqlx(type_name = "VARCHAR", rename_all = "lowercase")]
#[serde(rename_all = "lowercase")]
pub enum SessionStatus {
    #[default]
    Active,
    Completed,
    Aborted,
    Expired,
    Error,
}

/// Primary domain detected from DSL execution
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PrimaryDomain {
    Cbu,
    Kyc,
    Onboarding,
    Entity,
    Document,
    Custody,
}

/// Persisted session state
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PersistedSession {
    pub session_id: Uuid,
    pub status: SessionStatus,

    // Domain context
    pub primary_domain: Option<String>,
    pub cbu_id: Option<Uuid>,
    pub kyc_case_id: Option<Uuid>,
    pub onboarding_request_id: Option<Uuid>,

    // Bindings
    pub named_refs: HashMap<String, Uuid>,

    // Metadata
    pub client_type: Option<String>,
    pub jurisdiction: Option<String>,

    // Timing
    pub created_at: DateTime<Utc>,
    pub last_activity_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,

    // Error tracking
    pub error_count: i32,
    pub last_error: Option<String>,
    pub last_error_at: Option<DateTime<Utc>>,
}

/// DSL snapshot (successful execution)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DslSnapshot {
    pub snapshot_id: Uuid,
    pub session_id: Uuid,
    pub version: i32,
    pub dsl_source: String,
    pub dsl_checksum: String,
    pub success: bool,
    pub bindings_captured: HashMap<String, Uuid>,
    pub entities_created: Vec<EntityCreated>,
    pub domains_used: Vec<String>,
    pub executed_at: DateTime<Utc>,
    pub execution_ms: Option<i32>,
}

/// Entity created during execution
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EntityCreated {
    pub entity_type: String,
    pub entity_id: Uuid,
    pub name: Option<String>,
}

/// Session event types
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionEventType {
    Created,
    ExecuteStarted,
    ExecuteSuccess,
    ExecuteFailed,
    ValidationError,
    Timeout,
    Aborted,
    Expired,
    Completed,
    BindingAdded,
    DomainDetected,
    ErrorRecovered,
}

impl std::fmt::Display for SessionEventType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Created => write!(f, "created"),
            Self::ExecuteStarted => write!(f, "execute_started"),
            Self::ExecuteSuccess => write!(f, "execute_success"),
            Self::ExecuteFailed => write!(f, "execute_failed"),
            Self::ValidationError => write!(f, "validation_error"),
            Self::Timeout => write!(f, "timeout"),
            Self::Aborted => write!(f, "aborted"),
            Self::Expired => write!(f, "expired"),
            Self::Completed => write!(f, "completed"),
            Self::BindingAdded => write!(f, "binding_added"),
            Self::DomainDetected => write!(f, "domain_detected"),
            Self::ErrorRecovered => write!(f, "error_recovered"),
        }
    }
}

// ============================================================================
// Repository
// ============================================================================

/// Session persistence repository
pub struct SessionRepository {
    pool: PgPool,
}

impl SessionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    // ------------------------------------------------------------------------
    // Session CRUD
    // ------------------------------------------------------------------------

    /// Create a new session with a specific ID
    pub async fn create_session_with_id(
        &self,
        session_id: Uuid,
        client_type: Option<&str>,
        jurisdiction: Option<&str>,
    ) -> Result<PersistedSession, sqlx::Error> {
        let now = Utc::now();
        let expires_at = now + chrono::Duration::hours(24);

        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".dsl_sessions
                (session_id, status, client_type, jurisdiction, created_at, last_activity_at, expires_at, named_refs)
            VALUES ($1, 'active', $2, $3, $4, $4, $5, '{}'::jsonb)
            "#,
            session_id,
            client_type,
            jurisdiction,
            now,
            expires_at,
        )
        .execute(&self.pool)
        .await?;

        // Log creation event
        self.log_event(session_id, SessionEventType::Created, None, None, None)
            .await?;

        Ok(PersistedSession {
            session_id,
            status: SessionStatus::Active,
            primary_domain: None,
            cbu_id: None,
            kyc_case_id: None,
            onboarding_request_id: None,
            named_refs: HashMap::new(),
            client_type: client_type.map(String::from),
            jurisdiction: jurisdiction.map(String::from),
            created_at: now,
            last_activity_at: now,
            expires_at,
            completed_at: None,
            error_count: 0,
            last_error: None,
            last_error_at: None,
        })
    }

    /// Get a session by ID
    pub async fn get_session(
        &self,
        session_id: Uuid,
    ) -> Result<Option<PersistedSession>, sqlx::Error> {
        let row = sqlx::query!(
            r#"
            SELECT
                session_id, status, primary_domain, cbu_id, kyc_case_id, onboarding_request_id,
                named_refs, client_type, jurisdiction, created_at, last_activity_at, expires_at,
                completed_at, error_count, last_error, last_error_at
            FROM "ob-poc".dsl_sessions
            WHERE session_id = $1
            "#,
            session_id
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|r| {
            let named_refs: HashMap<String, Uuid> =
                serde_json::from_value(r.named_refs.clone()).unwrap_or_default();

            PersistedSession {
                session_id: r.session_id,
                status: match r.status.as_str() {
                    "active" => SessionStatus::Active,
                    "completed" => SessionStatus::Completed,
                    "aborted" => SessionStatus::Aborted,
                    "expired" => SessionStatus::Expired,
                    "error" => SessionStatus::Error,
                    _ => SessionStatus::Active,
                },
                primary_domain: r.primary_domain,
                cbu_id: r.cbu_id,
                kyc_case_id: r.kyc_case_id,
                onboarding_request_id: r.onboarding_request_id,
                named_refs,
                client_type: r.client_type,
                jurisdiction: r.jurisdiction,
                created_at: r.created_at,
                last_activity_at: r.last_activity_at,
                expires_at: r.expires_at,
                completed_at: r.completed_at,
                error_count: r.error_count,
                last_error: r.last_error,
                last_error_at: r.last_error_at,
            }
        }))
    }

    /// Update session activity timestamp
    pub async fn touch_session(&self, session_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            UPDATE "ob-poc".dsl_sessions
            SET last_activity_at = now()
            WHERE session_id = $1 AND status = 'active'
            "#,
            session_id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Update session bindings after successful execution
    pub async fn update_bindings(
        &self,
        session_id: Uuid,
        bindings: &HashMap<String, Uuid>,
        cbu_id: Option<Uuid>,
        kyc_case_id: Option<Uuid>,
        primary_domain: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        let bindings_json = serde_json::to_value(bindings).unwrap_or_default();

        sqlx::query!(
            r#"
            UPDATE "ob-poc".dsl_sessions
            SET
                named_refs = named_refs || $2::jsonb,
                cbu_id = COALESCE($3, cbu_id),
                kyc_case_id = COALESCE($4, kyc_case_id),
                primary_domain = COALESCE($5, primary_domain),
                last_activity_at = now()
            WHERE session_id = $1 AND status = 'active'
            "#,
            session_id,
            bindings_json,
            cbu_id,
            kyc_case_id,
            primary_domain,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Record an error on the session
    pub async fn record_error(&self, session_id: Uuid, error: &str) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            UPDATE "ob-poc".dsl_sessions
            SET
                error_count = error_count + 1,
                last_error = $2,
                last_error_at = now(),
                last_activity_at = now()
            WHERE session_id = $1
            "#,
            session_id,
            error,
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Mark session as completed
    pub async fn complete_session(&self, session_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            UPDATE "ob-poc".dsl_sessions
            SET status = 'completed', completed_at = now(), last_activity_at = now()
            WHERE session_id = $1 AND status = 'active'
            "#,
            session_id
        )
        .execute(&self.pool)
        .await?;

        self.log_event(session_id, SessionEventType::Completed, None, None, None)
            .await?;
        Ok(())
    }

    /// Mark session as aborted
    pub async fn abort_session(
        &self,
        session_id: Uuid,
        reason: Option<&str>,
    ) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"
            UPDATE "ob-poc".dsl_sessions
            SET status = 'aborted', last_error = $2, last_activity_at = now()
            WHERE session_id = $1 AND status = 'active'
            "#,
            session_id,
            reason,
        )
        .execute(&self.pool)
        .await?;

        self.log_event(session_id, SessionEventType::Aborted, None, reason, None)
            .await?;
        Ok(())
    }

    // ------------------------------------------------------------------------
    // Snapshots
    // ------------------------------------------------------------------------

    /// Save a DSL snapshot after successful execution
    pub async fn save_snapshot(
        &self,
        session_id: Uuid,
        dsl_source: &str,
        bindings: &HashMap<String, Uuid>,
        entities: &[EntityCreated],
        domains: &[String],
        execution_ms: Option<i32>,
    ) -> Result<DslSnapshot, sqlx::Error> {
        let snapshot_id = Uuid::new_v4();
        let checksum = compute_checksum(dsl_source);
        let now = Utc::now();

        // Get next version
        let version: i32 = sqlx::query_scalar!(
            r#"
            SELECT COALESCE(MAX(version), 0) + 1 as "version!"
            FROM "ob-poc".dsl_snapshots
            WHERE session_id = $1
            "#,
            session_id
        )
        .fetch_one(&self.pool)
        .await?;

        let bindings_json = serde_json::to_value(bindings).unwrap_or_default();
        let entities_json = serde_json::to_value(entities).unwrap_or_default();

        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".dsl_snapshots
                (snapshot_id, session_id, version, dsl_source, dsl_checksum, success,
                 bindings_captured, entities_created, domains_used, executed_at, execution_ms)
            VALUES ($1, $2, $3, $4, $5, true, $6, $7, $8, $9, $10)
            "#,
            snapshot_id,
            session_id,
            version,
            dsl_source,
            checksum,
            bindings_json,
            entities_json,
            domains as &[String],
            now,
            execution_ms,
        )
        .execute(&self.pool)
        .await?;

        Ok(DslSnapshot {
            snapshot_id,
            session_id,
            version,
            dsl_source: dsl_source.to_string(),
            dsl_checksum: checksum,
            success: true,
            bindings_captured: bindings.clone(),
            entities_created: entities.to_vec(),
            domains_used: domains.to_vec(),
            executed_at: now,
            execution_ms,
        })
    }

    /// Get all snapshots for a session
    pub async fn get_snapshots(&self, session_id: Uuid) -> Result<Vec<DslSnapshot>, sqlx::Error> {
        let rows = sqlx::query!(
            r#"
            SELECT
                snapshot_id, session_id, version, dsl_source, dsl_checksum, success,
                bindings_captured, entities_created, domains_used, executed_at, execution_ms
            FROM "ob-poc".dsl_snapshots
            WHERE session_id = $1
            ORDER BY version ASC
            "#,
            session_id
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| DslSnapshot {
                snapshot_id: r.snapshot_id,
                session_id: r.session_id,
                version: r.version,
                dsl_source: r.dsl_source,
                dsl_checksum: r.dsl_checksum,
                success: r.success,
                bindings_captured: serde_json::from_value(r.bindings_captured.clone())
                    .unwrap_or_default(),
                entities_created: serde_json::from_value(r.entities_created.clone())
                    .unwrap_or_default(),
                domains_used: r.domains_used,
                executed_at: r.executed_at,
                execution_ms: r.execution_ms,
            })
            .collect())
    }

    /// Get latest snapshot for a session
    pub async fn get_latest_snapshot(
        &self,
        session_id: Uuid,
    ) -> Result<Option<DslSnapshot>, sqlx::Error> {
        let snapshots = self.get_snapshots(session_id).await?;
        Ok(snapshots.into_iter().last())
    }

    // ------------------------------------------------------------------------
    // Events / Audit Log
    // ------------------------------------------------------------------------

    /// Log a session event
    pub async fn log_event(
        &self,
        session_id: Uuid,
        event_type: SessionEventType,
        dsl_source: Option<&str>,
        error_message: Option<&str>,
        metadata: Option<serde_json::Value>,
    ) -> Result<Uuid, sqlx::Error> {
        let event_id = Uuid::new_v4();
        let metadata = metadata.unwrap_or(serde_json::json!({}));

        sqlx::query!(
            r#"
            INSERT INTO "ob-poc".dsl_session_events
                (event_id, session_id, event_type, dsl_source, error_message, metadata, occurred_at)
            VALUES ($1, $2, $3, $4, $5, $6, now())
            "#,
            event_id,
            session_id,
            event_type.to_string(),
            dsl_source,
            error_message,
            metadata,
        )
        .execute(&self.pool)
        .await?;

        Ok(event_id)
    }

    // ------------------------------------------------------------------------
    // Locks (for timeout detection)
    // ------------------------------------------------------------------------

    /// Acquire a lock for an operation
    pub async fn acquire_lock(
        &self,
        session_id: Uuid,
        operation: &str,
        timeout_secs: i32,
    ) -> Result<bool, sqlx::Error> {
        let result = sqlx::query!(
            r#"
            INSERT INTO "ob-poc".dsl_session_locks (session_id, operation, locked_at, lock_timeout_at)
            VALUES ($1, $2, now(), now() + ($3 || ' seconds')::interval)
            ON CONFLICT (session_id) DO NOTHING
            "#,
            session_id,
            operation,
            timeout_secs.to_string(),
        )
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    /// Release a lock
    pub async fn release_lock(&self, session_id: Uuid) -> Result<(), sqlx::Error> {
        sqlx::query!(
            r#"DELETE FROM "ob-poc".dsl_session_locks WHERE session_id = $1"#,
            session_id
        )
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Check if session is locked
    pub async fn is_locked(&self, session_id: Uuid) -> Result<bool, sqlx::Error> {
        let locked = sqlx::query_scalar!(
            r#"
            SELECT EXISTS(
                SELECT 1 FROM "ob-poc".dsl_session_locks
                WHERE session_id = $1 AND lock_timeout_at > now()
            ) as "exists!"
            "#,
            session_id
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(locked)
    }

    // ------------------------------------------------------------------------
    // Cleanup
    // ------------------------------------------------------------------------

    /// Clean up expired sessions
    pub async fn cleanup_expired(&self) -> Result<i32, sqlx::Error> {
        let result =
            sqlx::query_scalar!(r#"SELECT "ob-poc".cleanup_expired_sessions() as "count!""#)
                .fetch_one(&self.pool)
                .await?;

        Ok(result)
    }

    /// Abort hung sessions (locked too long)
    pub async fn abort_hung_sessions(&self) -> Result<i32, sqlx::Error> {
        let result = sqlx::query_scalar!(r#"SELECT "ob-poc".abort_hung_sessions() as "count!""#)
            .fetch_one(&self.pool)
            .await?;

        Ok(result)
    }
}

// ============================================================================
// Helpers
// ============================================================================

/// Compute SHA256 checksum of DSL source
fn compute_checksum(dsl: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(dsl.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Detect primary domain from DSL verbs
pub fn detect_domain(dsl: &str) -> Option<String> {
    // Simple heuristic: look at verb prefixes
    if dsl.contains("onboarding.") {
        Some("onboarding".to_string())
    } else if dsl.contains("kyc-case.") || dsl.contains("entity-workstream.") {
        Some("kyc".to_string())
    } else if dsl.contains("cbu.") {
        Some("cbu".to_string())
    } else if dsl.contains("entity.") {
        Some("entity".to_string())
    } else if dsl.contains("cbu-custody.") {
        Some("custody".to_string())
    } else if dsl.contains("document.") {
        Some("document".to_string())
    } else {
        None
    }
}

/// Extract domains used in DSL
pub fn extract_domains(dsl: &str) -> Vec<String> {
    let mut domains = Vec::new();

    let domain_patterns = [
        ("cbu.", "cbu"),
        ("entity.", "entity"),
        ("kyc-case.", "kyc-case"),
        ("entity-workstream.", "entity-workstream"),
        ("document.", "document"),
        ("screening.", "screening"),
        ("ubo.", "ubo"),
        ("service-resource.", "service-resource"),
        ("cbu-custody.", "cbu-custody"),
        ("onboarding.", "onboarding"),
    ];

    for (pattern, domain) in domain_patterns {
        if dsl.contains(pattern) && !domains.contains(&domain.to_string()) {
            domains.push(domain.to_string());
        }
    }

    domains
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_checksum() {
        let dsl = r#"(cbu.ensure :name "Test" :jurisdiction "LU")"#;
        let checksum = compute_checksum(dsl);
        assert_eq!(checksum.len(), 64); // SHA256 hex = 64 chars

        // Same input = same checksum
        let checksum2 = compute_checksum(dsl);
        assert_eq!(checksum, checksum2);

        // Different input = different checksum
        let checksum3 = compute_checksum("different");
        assert_ne!(checksum, checksum3);
    }

    #[test]
    fn test_detect_domain() {
        assert_eq!(
            detect_domain("(cbu.ensure :name \"Test\")"),
            Some("cbu".to_string())
        );
        assert_eq!(
            detect_domain("(kyc-case.create :cbu-id @cbu)"),
            Some("kyc".to_string())
        );
        assert_eq!(
            detect_domain("(entity.create-limited-company :name \"X\")"),
            Some("entity".to_string())
        );
        assert_eq!(
            detect_domain("(onboarding.request :cbu-id @cbu)"),
            Some("onboarding".to_string())
        );
    }

    #[test]
    fn test_extract_domains() {
        let dsl = r#"
            (cbu.ensure :name "Test" :as @cbu)
            (entity.create-limited-company :name "X" :as @company)
            (kyc-case.create :cbu-id @cbu :as @case)
        "#;

        let domains = extract_domains(dsl);
        assert!(domains.contains(&"cbu".to_string()));
        assert!(domains.contains(&"entity".to_string()));
        assert!(domains.contains(&"kyc-case".to_string()));
    }
}
