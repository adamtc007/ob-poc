//! Catalogue authorship verb implementations — Tranche 3 Phase 3.B.
//!
//! v1.2 §8 Tranche 3 — governed authorship mechanism. The four verbs
//! here replace the v1.2 §P.8 prototype stubs from earlier in the
//! migration; they are the architectural surface through which the verb
//! catalogue is amended.
//!
//! ## State machine (per `catalogue_dag.yaml`)
//!
//! ```text
//! (entry) → DRAFT          via propose-verb-declaration
//! DRAFT   → STAGED         via stage-proposal           (auto on validator-clean)
//! STAGED  → COMMITTED      via commit-verb-declaration  (requires_explicit_authorisation)
//! STAGED  → ROLLED_BACK    via rollback-verb-declaration
//! ```
//!
//! ## Two-eye rule
//!
//! `commit-verb-declaration` enforces that the committing principal is
//! different from the proposing principal (the `catalogue_two_eye_rule`
//! CHECK constraint on `catalogue_proposals` backs this at the DB layer;
//! the verb handler checks it pre-flight to give a clean error).
//!
//! ## Forward-discipline
//!
//! In Phase 3.F Stage 1 (this session), COMMITTED writes a row to
//! `catalogue_committed_verbs` but does NOT write back to `config/verbs/`
//! YAML. The YAML remains the source of truth at this stage. Stages 2-4
//! progressively enforce the database-as-source-of-truth.
//!
//! See `docs/governance/tranche-3-design-2026-04-26.md`.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use sem_os_postgres::ops::SemOsVerbOp;
use serde_json::json;
use sqlx::Row;
use uuid::Uuid;

use dsl_runtime::domain_ops::helpers::{
    json_extract_string, json_extract_string_opt, json_extract_uuid,
};
use dsl_runtime::tx::TransactionScope;
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

// ---------------------------------------------------------------------------
// catalogue.propose-verb-declaration
// ---------------------------------------------------------------------------

pub struct CataloguePropose;

#[async_trait]
impl SemOsVerbOp for CataloguePropose {
    fn fqn(&self) -> &str {
        "catalogue.propose-verb-declaration"
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let verb_fqn = json_extract_string(args, "verb-fqn")?;
        let proposed = args
            .get("proposed-declaration")
            .ok_or_else(|| anyhow!("Missing proposed-declaration argument"))?;
        let rationale = json_extract_string_opt(args, "rationale");
        // The proposing principal is sourced from the verb context.
        let proposed_by = ctx.principal.actor_id.clone();

        let proposal_id: Uuid = sqlx::query_scalar(
            r#"INSERT INTO "ob-poc".catalogue_proposals
               (verb_fqn, proposed_declaration, rationale, status, proposed_by)
               VALUES ($1, $2, $3, 'DRAFT', $4)
               RETURNING proposal_id"#,
        )
        .bind(&verb_fqn)
        .bind(proposed)
        .bind(&rationale)
        .bind(&proposed_by)
        .fetch_one(scope.executor())
        .await?;

        ctx.bind("proposal", proposal_id);
        Ok(VerbExecutionOutcome::Uuid(proposal_id))
    }
}

// ---------------------------------------------------------------------------
// catalogue.commit-verb-declaration
// ---------------------------------------------------------------------------

pub struct CatalogueCommit;

#[async_trait]
impl SemOsVerbOp for CatalogueCommit {
    fn fqn(&self) -> &str {
        "catalogue.commit-verb-declaration"
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let proposal_id: Uuid = json_extract_uuid(args, ctx, "proposal")?;
        let approver = json_extract_string(args, "approver")?;
        let acting_principal = ctx.principal.actor_id.clone();

        // Two-eye rule: the principal invoking commit must match the
        // approver arg AND must differ from the proposer.
        if approver != acting_principal {
            return Err(anyhow!(
                "Two-eye rule violation: approver arg ({}) must match invoking principal ({})",
                approver,
                acting_principal
            ));
        }

        // Pull current state + proposed_by + declaration for the projection write.
        let row = sqlx::query(
            r#"SELECT status, proposed_by, verb_fqn, proposed_declaration
               FROM "ob-poc".catalogue_proposals WHERE proposal_id = $1"#,
        )
        .bind(proposal_id)
        .fetch_optional(scope.executor())
        .await?
        .ok_or_else(|| anyhow!("Proposal not found: {}", proposal_id))?;

        let status: String = row.try_get("status")?;
        let proposed_by: String = row.try_get("proposed_by")?;
        let verb_fqn: String = row.try_get("verb_fqn")?;
        let declaration: serde_json::Value = row.try_get("proposed_declaration")?;

        if status != "STAGED" {
            return Err(anyhow!(
                "Proposal {} cannot be committed from state '{}'; must be STAGED",
                proposal_id,
                status
            ));
        }
        if proposed_by == acting_principal {
            return Err(anyhow!(
                "Two-eye rule violation: committer ({}) must differ from proposer ({})",
                acting_principal,
                proposed_by
            ));
        }

        // Atomic transition + projection write.
        sqlx::query(
            r#"UPDATE "ob-poc".catalogue_proposals
               SET status = 'COMMITTED', committed_by = $2, committed_at = now()
               WHERE proposal_id = $1"#,
        )
        .bind(proposal_id)
        .bind(&acting_principal)
        .execute(scope.executor())
        .await?;

        sqlx::query(
            r#"INSERT INTO "ob-poc".catalogue_committed_verbs
               (verb_fqn, declaration, committed_proposal_id)
               VALUES ($1, $2, $3)
               ON CONFLICT (verb_fqn) DO UPDATE
                 SET declaration = EXCLUDED.declaration,
                     committed_proposal_id = EXCLUDED.committed_proposal_id,
                     committed_at = now()"#,
        )
        .bind(&verb_fqn)
        .bind(&declaration)
        .bind(proposal_id)
        .execute(scope.executor())
        .await?;

        Ok(VerbExecutionOutcome::Record(json!({
            "proposal_id": proposal_id,
            "verb_fqn": verb_fqn,
            "status": "COMMITTED",
            "committed_by": acting_principal,
        })))
    }
}

// ---------------------------------------------------------------------------
// catalogue.rollback-verb-declaration
// ---------------------------------------------------------------------------

pub struct CatalogueRollback;

#[async_trait]
impl SemOsVerbOp for CatalogueRollback {
    fn fqn(&self) -> &str {
        "catalogue.rollback-verb-declaration"
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let proposal_id: Uuid = json_extract_uuid(args, ctx, "proposal")?;
        let reason = json_extract_string_opt(args, "reason");
        let acting_principal = ctx.principal.actor_id.clone();

        let current_status: Option<String> = sqlx::query_scalar(
            r#"SELECT status FROM "ob-poc".catalogue_proposals WHERE proposal_id = $1"#,
        )
        .bind(proposal_id)
        .fetch_optional(scope.executor())
        .await?;

        let status =
            current_status.ok_or_else(|| anyhow!("Proposal not found: {}", proposal_id))?;
        if status != "STAGED" && status != "DRAFT" {
            return Err(anyhow!(
                "Proposal {} cannot be rolled back from state '{}'; must be DRAFT or STAGED",
                proposal_id,
                status
            ));
        }

        let affected = sqlx::query(
            r#"UPDATE "ob-poc".catalogue_proposals
               SET status = 'ROLLED_BACK',
                   rolled_back_by = $2,
                   rolled_back_at = now(),
                   rolled_back_reason = $3
               WHERE proposal_id = $1"#,
        )
        .bind(proposal_id)
        .bind(&acting_principal)
        .bind(&reason)
        .execute(scope.executor())
        .await?
        .rows_affected();

        Ok(VerbExecutionOutcome::Affected(affected))
    }
}

// ---------------------------------------------------------------------------
// catalogue.list-proposals
// ---------------------------------------------------------------------------

pub struct CatalogueListProposals;

#[async_trait]
impl SemOsVerbOp for CatalogueListProposals {
    fn fqn(&self) -> &str {
        "catalogue.list-proposals"
    }

    async fn execute(
        &self,
        args: &serde_json::Value,
        _ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let status_filter =
            json_extract_string_opt(args, "status-filter").unwrap_or_else(|| "pending".to_string());

        let sql = match status_filter.as_str() {
            "pending" => {
                r#"SELECT proposal_id, verb_fqn, status, proposed_by, created_at
                   FROM "ob-poc".catalogue_proposals
                   WHERE status IN ('DRAFT', 'STAGED')
                   ORDER BY created_at DESC LIMIT 100"#
            }
            "committed" => {
                r#"SELECT proposal_id, verb_fqn, status, proposed_by, committed_at AS created_at
                   FROM "ob-poc".catalogue_proposals
                   WHERE status = 'COMMITTED'
                   ORDER BY committed_at DESC LIMIT 100"#
            }
            "rolled_back" => {
                r#"SELECT proposal_id, verb_fqn, status, proposed_by, rolled_back_at AS created_at
                   FROM "ob-poc".catalogue_proposals
                   WHERE status = 'ROLLED_BACK'
                   ORDER BY rolled_back_at DESC LIMIT 100"#
            }
            _ => {
                // "all" or anything else
                r#"SELECT proposal_id, verb_fqn, status, proposed_by, created_at
                   FROM "ob-poc".catalogue_proposals
                   ORDER BY created_at DESC LIMIT 100"#
            }
        };

        let rows = sqlx::query(sql).fetch_all(scope.executor()).await?;
        let mut out = Vec::with_capacity(rows.len());
        for r in rows {
            let proposal_id: Uuid = r.try_get("proposal_id")?;
            let verb_fqn: String = r.try_get("verb_fqn")?;
            let status: String = r.try_get("status")?;
            let proposed_by: String = r.try_get("proposed_by")?;
            let created_at: chrono::DateTime<chrono::Utc> = r.try_get("created_at")?;
            out.push(json!({
                "proposal_id": proposal_id,
                "verb_fqn": verb_fqn,
                "status": status,
                "proposed_by": proposed_by,
                "created_at": created_at,
            }));
        }
        Ok(VerbExecutionOutcome::RecordSet(out))
    }
}
