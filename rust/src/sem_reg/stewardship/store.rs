//! StewardshipStore — DB operations for `sem_reg.*` stewardship tables.
//!
//! Follows the unit-struct pattern from `SnapshotStore`.
//! All event writes are append-only (immutable audit chain).

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::types::*;

/// Database operations for stewardship-layer tables.
pub(crate) struct StewardshipStore;

impl StewardshipStore {
    // ─── Events (§9.4) ─────────────────────────────────────────

    /// Append an immutable event to the stewardship audit chain.
    pub(crate) async fn append_event(pool: &PgPool, record: &StewardshipRecord) -> Result<()> {
        let payload = serde_json::to_value(&record.event_type)?;
        sqlx::query(
            r#"
            INSERT INTO sem_reg.events (
                event_id, changeset_id, event_type, actor_id,
                payload, viewport_manifest_id, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(record.event_id)
        .bind(record.changeset_id)
        .bind(record.event_type.db_event_type())
        .bind(&record.actor_id)
        .bind(&payload)
        .bind(record.viewport_manifest_id)
        .bind(record.created_at)
        .execute(pool)
        .await?;
        Ok(())
    }

    // ─── Basis (§9.3) ──────────────────────────────────────────

    /// Insert a basis record.
    pub(crate) async fn insert_basis(pool: &PgPool, basis: &BasisRecord) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO sem_reg.basis_records (
                basis_id, changeset_id, entry_id, kind,
                title, narrative, created_by, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(basis.basis_id)
        .bind(basis.changeset_id)
        .bind(basis.entry_id)
        .bind(basis.kind.as_str())
        .bind(&basis.title)
        .bind(&basis.narrative)
        .bind(&basis.created_by)
        .bind(basis.created_at)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// List basis records for a changeset.
    pub(crate) async fn list_basis(pool: &PgPool, changeset_id: Uuid) -> Result<Vec<BasisRecord>> {
        let rows = sqlx::query_as::<_, BasisRow>(
            r#"
            SELECT basis_id, changeset_id, entry_id, kind,
                   title, narrative, created_by, created_at
            FROM sem_reg.basis_records
            WHERE changeset_id = $1
            ORDER BY created_at ASC
            "#,
        )
        .bind(changeset_id)
        .fetch_all(pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into_record()).collect())
    }

    /// Insert a basis claim.
    pub(crate) async fn insert_claim(pool: &PgPool, claim: &BasisClaim) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO sem_reg.basis_claims (
                claim_id, basis_id, claim_text, reference_uri,
                excerpt, confidence, flagged_as_open_question
            ) VALUES ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(claim.claim_id)
        .bind(claim.basis_id)
        .bind(&claim.claim_text)
        .bind(&claim.reference_uri)
        .bind(&claim.excerpt)
        .bind(claim.confidence)
        .bind(claim.flagged_as_open_question)
        .execute(pool)
        .await?;
        Ok(())
    }

    // ─── Conflicts (§9.6) ──────────────────────────────────────

    /// List conflicts for a changeset.
    pub(crate) async fn list_conflicts(pool: &PgPool, changeset_id: Uuid) -> Result<Vec<ConflictRecord>> {
        let rows = sqlx::query_as::<_, ConflictRow>(
            r#"
            SELECT conflict_id, changeset_id, competing_changeset_id,
                   fqn, detected_at, resolution_strategy,
                   resolution_rationale, resolved_by, resolved_at
            FROM sem_reg.conflict_records
            WHERE changeset_id = $1
            ORDER BY detected_at ASC
            "#,
        )
        .bind(changeset_id)
        .fetch_all(pool)
        .await?;

        Ok(rows.into_iter().map(|r| r.into_record()).collect())
    }

    /// Resolve a conflict with a strategy.
    pub(crate) async fn resolve_conflict(
        pool: &PgPool,
        conflict_id: Uuid,
        strategy: ConflictStrategy,
        rationale: &str,
        actor: &str,
    ) -> Result<()> {
        let rows = sqlx::query(
            r#"
            UPDATE sem_reg.conflict_records
            SET resolution_strategy = $2,
                resolution_rationale = $3,
                resolved_by = $4,
                resolved_at = now()
            WHERE conflict_id = $1
            "#,
        )
        .bind(conflict_id)
        .bind(strategy.as_str())
        .bind(rationale)
        .bind(actor)
        .execute(pool)
        .await?;

        if rows.rows_affected() == 0 {
            return Err(anyhow!("Conflict {} not found", conflict_id));
        }
        Ok(())
    }

    // ─── Templates (§9.5) ──────────────────────────────────────

    /// Get the active template for a given FQN.
    pub(crate) async fn get_active_template(
        pool: &PgPool,
        fqn: &str,
    ) -> Result<Option<StewardshipTemplate>> {
        let row = sqlx::query_as::<_, TemplateRow>(
            r#"
            SELECT template_id, fqn, display_name,
                   version_major, version_minor, version_patch,
                   domain, scope, items, steward, basis_ref,
                   status, created_by, created_at
            FROM sem_reg.templates
            WHERE fqn = $1 AND status = 'active'
            LIMIT 1
            "#,
        )
        .bind(fqn)
        .fetch_optional(pool)
        .await?;

        row.map(|r| r.into_template()).transpose()
    }

    // ─── Idempotency (§6.2) ────────────────────────────────────

    /// Check if a client_request_id has already been processed.
    pub(crate) async fn check_idempotency(
        pool: &PgPool,
        client_request_id: Uuid,
    ) -> Result<Option<serde_json::Value>> {
        let row = sqlx::query_scalar::<_, serde_json::Value>(
            r#"
            SELECT result FROM sem_reg.idempotency_keys
            WHERE client_request_id = $1
            "#,
        )
        .bind(client_request_id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    /// Record an idempotency key with the tool result.
    pub(crate) async fn record_idempotency(
        pool: &PgPool,
        client_request_id: Uuid,
        tool_name: &str,
        result: &serde_json::Value,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO sem_reg.idempotency_keys (
                client_request_id, tool_name, result
            ) VALUES ($1, $2, $3)
            ON CONFLICT (client_request_id) DO NOTHING
            "#,
        )
        .bind(client_request_id)
        .bind(tool_name)
        .bind(result)
        .execute(pool)
        .await?;
        Ok(())
    }

}

// ═══════════════════════════════════════════════════════════════════
//  Internal row types for sqlx::FromRow
// ═══════════════════════════════════════════════════════════════════

#[derive(sqlx::FromRow)]
struct BasisRow {
    basis_id: Uuid,
    changeset_id: Uuid,
    entry_id: Option<Uuid>,
    kind: String,
    title: String,
    narrative: Option<String>,
    created_by: String,
    created_at: DateTime<Utc>,
}

impl BasisRow {
    fn into_record(self) -> BasisRecord {
        BasisRecord {
            basis_id: self.basis_id,
            changeset_id: self.changeset_id,
            entry_id: self.entry_id,
            kind: BasisKind::parse(&self.kind).unwrap_or(BasisKind::Precedent),
            title: self.title,
            narrative: self.narrative,
            created_by: self.created_by,
            created_at: self.created_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct ConflictRow {
    conflict_id: Uuid,
    changeset_id: Uuid,
    competing_changeset_id: Uuid,
    fqn: String,
    detected_at: DateTime<Utc>,
    resolution_strategy: Option<String>,
    resolution_rationale: Option<String>,
    resolved_by: Option<String>,
    resolved_at: Option<DateTime<Utc>>,
}

impl ConflictRow {
    fn into_record(self) -> ConflictRecord {
        ConflictRecord {
            conflict_id: self.conflict_id,
            changeset_id: self.changeset_id,
            competing_changeset_id: self.competing_changeset_id,
            fqn: self.fqn,
            detected_at: self.detected_at,
            resolution_strategy: self
                .resolution_strategy
                .and_then(|s| ConflictStrategy::parse(&s)),
            resolution_rationale: self.resolution_rationale,
            resolved_by: self.resolved_by,
            resolved_at: self.resolved_at,
        }
    }
}

#[derive(sqlx::FromRow)]
struct TemplateRow {
    template_id: Uuid,
    fqn: String,
    display_name: String,
    version_major: i32,
    version_minor: i32,
    version_patch: i32,
    domain: String,
    scope: serde_json::Value,
    items: serde_json::Value,
    steward: String,
    basis_ref: Option<Uuid>,
    status: String,
    created_by: String,
    created_at: DateTime<Utc>,
}

impl TemplateRow {
    fn into_template(self) -> Result<StewardshipTemplate> {
        let scope: Vec<String> = serde_json::from_value(self.scope)?;
        let items: Vec<TemplateItem> = serde_json::from_value(self.items)?;

        Ok(StewardshipTemplate {
            template_id: self.template_id,
            fqn: self.fqn,
            display_name: self.display_name,
            version: SemanticVersion {
                major: self.version_major as u32,
                minor: self.version_minor as u32,
                patch: self.version_patch as u32,
            },
            domain: self.domain,
            scope,
            items,
            steward: self.steward,
            basis_ref: self.basis_ref,
            status: TemplateStatus::parse(&self.status).unwrap_or(TemplateStatus::Draft),
            created_by: self.created_by,
            created_at: self.created_at,
        })
    }
}

