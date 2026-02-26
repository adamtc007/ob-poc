//! StewardshipStore — DB operations for `stewardship.*` tables.
//!
//! Follows the unit-struct pattern from `SnapshotStore`.
//! All event writes are append-only (immutable audit chain).

use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

use super::types::*;

/// Database operations for stewardship-layer tables.
pub struct StewardshipStore;

impl StewardshipStore {
    // ─── Events (§9.4) ─────────────────────────────────────────

    /// Append an immutable event to the stewardship audit chain.
    pub async fn append_event(pool: &PgPool, record: &StewardshipRecord) -> Result<()> {
        let payload = serde_json::to_value(&record.event_type)?;
        sqlx::query(
            r#"
            INSERT INTO stewardship.events (
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

    /// List events for a changeset, ordered by creation time.
    pub async fn list_events(
        pool: &PgPool,
        changeset_id: Uuid,
        limit: i64,
    ) -> Result<Vec<StewardshipRecord>> {
        let rows = sqlx::query_as::<_, EventRow>(
            r#"
            SELECT event_id, changeset_id, event_type, actor_id,
                   payload, viewport_manifest_id, created_at
            FROM stewardship.events
            WHERE changeset_id = $1
            ORDER BY created_at ASC
            LIMIT $2
            "#,
        )
        .bind(changeset_id)
        .bind(limit)
        .fetch_all(pool)
        .await?;

        rows.into_iter().map(|r| r.into_record()).collect()
    }

    // ─── Basis (§9.3) ──────────────────────────────────────────

    /// Insert a basis record.
    pub async fn insert_basis(pool: &PgPool, basis: &BasisRecord) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO stewardship.basis_records (
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
    pub async fn list_basis(pool: &PgPool, changeset_id: Uuid) -> Result<Vec<BasisRecord>> {
        let rows = sqlx::query_as::<_, BasisRow>(
            r#"
            SELECT basis_id, changeset_id, entry_id, kind,
                   title, narrative, created_by, created_at
            FROM stewardship.basis_records
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
    pub async fn insert_claim(pool: &PgPool, claim: &BasisClaim) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO stewardship.basis_claims (
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

    /// List claims for a basis record.
    pub async fn list_claims(pool: &PgPool, basis_id: Uuid) -> Result<Vec<BasisClaim>> {
        let rows = sqlx::query_as::<_, ClaimRow>(
            r#"
            SELECT claim_id, basis_id, claim_text, reference_uri,
                   excerpt, confidence, flagged_as_open_question
            FROM stewardship.basis_claims
            WHERE basis_id = $1
            ORDER BY created_at ASC
            "#,
        )
        .bind(basis_id)
        .fetch_all(pool)
        .await?;

        Ok(rows
            .into_iter()
            .map(|r| BasisClaim {
                claim_id: r.claim_id,
                basis_id: r.basis_id,
                claim_text: r.claim_text,
                reference_uri: r.reference_uri,
                excerpt: r.excerpt,
                confidence: r.confidence,
                flagged_as_open_question: r.flagged_as_open_question,
            })
            .collect())
    }

    // ─── Conflicts (§9.6) ──────────────────────────────────────

    /// Insert a conflict record.
    pub async fn insert_conflict(pool: &PgPool, conflict: &ConflictRecord) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO stewardship.conflict_records (
                conflict_id, changeset_id, competing_changeset_id,
                fqn, detected_at
            ) VALUES ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(conflict.conflict_id)
        .bind(conflict.changeset_id)
        .bind(conflict.competing_changeset_id)
        .bind(&conflict.fqn)
        .bind(conflict.detected_at)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// List conflicts for a changeset.
    pub async fn list_conflicts(pool: &PgPool, changeset_id: Uuid) -> Result<Vec<ConflictRecord>> {
        let rows = sqlx::query_as::<_, ConflictRow>(
            r#"
            SELECT conflict_id, changeset_id, competing_changeset_id,
                   fqn, detected_at, resolution_strategy,
                   resolution_rationale, resolved_by, resolved_at
            FROM stewardship.conflict_records
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
    pub async fn resolve_conflict(
        pool: &PgPool,
        conflict_id: Uuid,
        strategy: ConflictStrategy,
        rationale: &str,
        actor: &str,
    ) -> Result<()> {
        let rows = sqlx::query(
            r#"
            UPDATE stewardship.conflict_records
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

    /// Insert a template.
    pub async fn insert_template(pool: &PgPool, template: &StewardshipTemplate) -> Result<()> {
        let scope_json = serde_json::to_value(&template.scope)?;
        let items_json = serde_json::to_value(&template.items)?;

        sqlx::query(
            r#"
            INSERT INTO stewardship.templates (
                template_id, fqn, display_name,
                version_major, version_minor, version_patch,
                domain, scope, items, steward, basis_ref,
                status, created_by, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
            "#,
        )
        .bind(template.template_id)
        .bind(&template.fqn)
        .bind(&template.display_name)
        .bind(template.version.major as i32)
        .bind(template.version.minor as i32)
        .bind(template.version.patch as i32)
        .bind(&template.domain)
        .bind(&scope_json)
        .bind(&items_json)
        .bind(&template.steward)
        .bind(template.basis_ref)
        .bind(template.status.as_str())
        .bind(&template.created_by)
        .bind(template.created_at)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Get the active template for a given FQN.
    pub async fn get_active_template(
        pool: &PgPool,
        fqn: &str,
    ) -> Result<Option<StewardshipTemplate>> {
        let row = sqlx::query_as::<_, TemplateRow>(
            r#"
            SELECT template_id, fqn, display_name,
                   version_major, version_minor, version_patch,
                   domain, scope, items, steward, basis_ref,
                   status, created_by, created_at
            FROM stewardship.templates
            WHERE fqn = $1 AND status = 'active'
            LIMIT 1
            "#,
        )
        .bind(fqn)
        .fetch_optional(pool)
        .await?;

        row.map(|r| r.into_template()).transpose()
    }

    /// List templates, optionally filtered by status.
    pub async fn list_templates(
        pool: &PgPool,
        status: Option<TemplateStatus>,
    ) -> Result<Vec<StewardshipTemplate>> {
        let rows = if let Some(s) = status {
            sqlx::query_as::<_, TemplateRow>(
                r#"
                SELECT template_id, fqn, display_name,
                       version_major, version_minor, version_patch,
                       domain, scope, items, steward, basis_ref,
                       status, created_by, created_at
                FROM stewardship.templates
                WHERE status = $1
                ORDER BY fqn ASC
                "#,
            )
            .bind(s.as_str())
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as::<_, TemplateRow>(
                r#"
                SELECT template_id, fqn, display_name,
                       version_major, version_minor, version_patch,
                       domain, scope, items, steward, basis_ref,
                       status, created_by, created_at
                FROM stewardship.templates
                ORDER BY fqn ASC
                "#,
            )
            .fetch_all(pool)
            .await?
        };

        rows.into_iter().map(|r| r.into_template()).collect()
    }

    // ─── Verb Bindings (§9.7) ──────────────────────────────────

    /// Insert a verb implementation binding.
    pub async fn insert_binding(pool: &PgPool, binding: &VerbImplementationBinding) -> Result<()> {
        let exec_modes_json = serde_json::to_value(&binding.exec_modes)?;

        sqlx::query(
            r#"
            INSERT INTO stewardship.verb_implementation_bindings (
                binding_id, verb_fqn, binding_kind, binding_ref,
                exec_modes, status, last_verified_at, notes
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(binding.binding_id)
        .bind(&binding.verb_fqn)
        .bind(binding.binding_kind.as_str())
        .bind(&binding.binding_ref)
        .bind(&exec_modes_json)
        .bind(binding.status.as_str())
        .bind(binding.last_verified_at)
        .bind(&binding.notes)
        .execute(pool)
        .await?;
        Ok(())
    }

    /// Get the active binding for a verb FQN.
    pub async fn get_active_binding(
        pool: &PgPool,
        verb_fqn: &str,
    ) -> Result<Option<VerbImplementationBinding>> {
        let row = sqlx::query_as::<_, BindingRow>(
            r#"
            SELECT binding_id, verb_fqn, binding_kind, binding_ref,
                   exec_modes, status, last_verified_at, notes
            FROM stewardship.verb_implementation_bindings
            WHERE verb_fqn = $1 AND status = 'active'
            LIMIT 1
            "#,
        )
        .bind(verb_fqn)
        .fetch_optional(pool)
        .await?;

        row.map(|r| r.into_binding()).transpose()
    }

    /// List bindings, optionally filtered by status.
    pub async fn list_bindings(
        pool: &PgPool,
        status: Option<BindingStatus>,
    ) -> Result<Vec<VerbImplementationBinding>> {
        let rows = if let Some(s) = status {
            sqlx::query_as::<_, BindingRow>(
                r#"
                SELECT binding_id, verb_fqn, binding_kind, binding_ref,
                       exec_modes, status, last_verified_at, notes
                FROM stewardship.verb_implementation_bindings
                WHERE status = $1
                ORDER BY verb_fqn ASC
                "#,
            )
            .bind(s.as_str())
            .fetch_all(pool)
            .await?
        } else {
            sqlx::query_as::<_, BindingRow>(
                r#"
                SELECT binding_id, verb_fqn, binding_kind, binding_ref,
                       exec_modes, status, last_verified_at, notes
                FROM stewardship.verb_implementation_bindings
                ORDER BY verb_fqn ASC
                "#,
            )
            .fetch_all(pool)
            .await?
        };

        rows.into_iter().map(|r| r.into_binding()).collect()
    }

    // ─── Idempotency (§6.2) ────────────────────────────────────

    /// Check if a client_request_id has already been processed.
    pub async fn check_idempotency(
        pool: &PgPool,
        client_request_id: Uuid,
    ) -> Result<Option<serde_json::Value>> {
        let row = sqlx::query_scalar::<_, serde_json::Value>(
            r#"
            SELECT result FROM stewardship.idempotency_keys
            WHERE client_request_id = $1
            "#,
        )
        .bind(client_request_id)
        .fetch_optional(pool)
        .await?;
        Ok(row)
    }

    /// Record an idempotency key with the tool result.
    pub async fn record_idempotency(
        pool: &PgPool,
        client_request_id: Uuid,
        tool_name: &str,
        result: &serde_json::Value,
    ) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO stewardship.idempotency_keys (
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

    // ─── Viewport Manifests (Phase 1) ──────────────────────────

    /// Insert a viewport manifest (immutable audit record).
    pub async fn insert_viewport_manifest(
        pool: &PgPool,
        manifest: &ViewportManifest,
    ) -> Result<()> {
        let focus_json = serde_json::to_value(&manifest.focus_state)?;
        let overlay_str = match &manifest.overlay_mode {
            OverlayMode::ActiveOnly => "active_only",
            OverlayMode::DraftOverlay { .. } => "draft_overlay",
        };
        let refs_json = serde_json::to_value(&manifest.rendered_viewports)?;

        sqlx::query(
            r#"
            INSERT INTO stewardship.viewport_manifests (
                manifest_id, session_id, changeset_id,
                focus_state, overlay_mode, assumed_principal,
                viewport_refs, created_at
            ) VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
            "#,
        )
        .bind(manifest.manifest_id)
        .bind(manifest.session_id)
        .bind(manifest.changeset_id)
        .bind(&focus_json)
        .bind(overlay_str)
        .bind(&manifest.assumed_principal)
        .bind(&refs_json)
        .bind(manifest.captured_at)
        .execute(pool)
        .await?;
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════
//  Internal row types for sqlx::FromRow
// ═══════════════════════════════════════════════════════════════════

#[derive(sqlx::FromRow)]
#[allow(dead_code)]
struct EventRow {
    event_id: Uuid,
    changeset_id: Uuid,
    event_type: String,
    actor_id: String,
    payload: serde_json::Value,
    viewport_manifest_id: Option<Uuid>,
    created_at: DateTime<Utc>,
}

impl EventRow {
    fn into_record(self) -> Result<StewardshipRecord> {
        let event_type: StewardshipEventType = serde_json::from_value(self.payload.clone())?;
        Ok(StewardshipRecord {
            event_id: self.event_id,
            changeset_id: self.changeset_id,
            event_type,
            actor_id: self.actor_id,
            payload: self.payload,
            viewport_manifest_id: self.viewport_manifest_id,
            created_at: self.created_at,
        })
    }
}

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
struct ClaimRow {
    claim_id: Uuid,
    basis_id: Uuid,
    claim_text: String,
    reference_uri: Option<String>,
    excerpt: Option<String>,
    confidence: Option<f64>,
    flagged_as_open_question: bool,
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

#[derive(sqlx::FromRow)]
struct BindingRow {
    binding_id: Uuid,
    verb_fqn: String,
    binding_kind: String,
    binding_ref: String,
    exec_modes: serde_json::Value,
    status: String,
    last_verified_at: Option<DateTime<Utc>>,
    notes: Option<String>,
}

impl BindingRow {
    fn into_binding(self) -> Result<VerbImplementationBinding> {
        let exec_modes: Vec<String> = serde_json::from_value(self.exec_modes)?;

        Ok(VerbImplementationBinding {
            binding_id: self.binding_id,
            verb_fqn: self.verb_fqn,
            binding_kind: BindingKind::parse(&self.binding_kind)
                .unwrap_or(BindingKind::RustHandler),
            binding_ref: self.binding_ref,
            exec_modes,
            status: BindingStatus::parse(&self.status).unwrap_or(BindingStatus::Draft),
            last_verified_at: self.last_verified_at,
            notes: self.notes,
        })
    }
}
