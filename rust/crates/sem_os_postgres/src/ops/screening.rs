//! Screening domain verbs (4 plugin verbs) — SemOS-side YAML-first
//! re-implementation of the plugin subset of
//! `rust/config/verbs/screening.yaml`.
//!
//! - `screening.pep` / `screening.sanctions` — idempotent enqueue of
//!   a PENDING screening row against the entity's active workstream.
//!   If a PENDING row already exists for that `(workstream, type)`
//!   pair, the existing id is bound + returned (no duplicate).
//! - `screening.adverse-media` — stub until the external service
//!   integration lands.
//! - `screening.bulk-refresh` — enqueues SANCTIONS + PEP +
//!   ADVERSE_MEDIA PENDING rows for every workstream in a case,
//!   `NOT EXISTS` guard so re-runs are safe.
//!
//! Slice #10 pattern applied: `sqlx::query!` macros rewritten as
//! runtime `sqlx::query_as` / `sqlx::query` so we dodge the
//! sqlx-offline cache entirely.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use dsl_runtime::TransactionScope;
use dsl_runtime::{json_extract_string_opt, json_extract_uuid};
use dsl_runtime::{VerbExecutionContext, VerbExecutionOutcome};

use super::SemOsVerbOp;

/// Enqueue a PENDING screening for an entity's active workstream.
/// Returns the existing PENDING screening if one already exists,
/// otherwise inserts a new one. Binds `:screening` in ctx.
async fn enqueue_workstream_screening(
    ctx: &mut VerbExecutionContext,
    scope: &mut dyn TransactionScope,
    entity_id: Uuid,
    screening_type: &str,
) -> Result<Uuid> {
    let workstream: Option<(Uuid,)> = sqlx::query_as(
        r#"SELECT w.workstream_id FROM "ob-poc".entity_workstreams w
           JOIN "ob-poc".cases c ON c.case_id = w.case_id
           WHERE w.entity_id = $1 AND w.status NOT IN ('COMPLETE', 'BLOCKED')
           ORDER BY w.created_at DESC
           LIMIT 1"#,
    )
    .bind(entity_id)
    .fetch_optional(scope.executor())
    .await?;

    let workstream_id = workstream.map(|(id,)| id).ok_or_else(|| {
        anyhow!("No active workstream for entity. Use case-screening.initiate instead.")
    })?;

    let existing: Option<(Uuid,)> = sqlx::query_as(
        r#"SELECT screening_id FROM "ob-poc".screenings
           WHERE workstream_id = $1 AND screening_type = $2 AND status = 'PENDING'
           LIMIT 1"#,
    )
    .bind(workstream_id)
    .bind(screening_type)
    .fetch_optional(scope.executor())
    .await?;

    if let Some((screening_id,)) = existing {
        ctx.bind("screening", screening_id);
        return Ok(screening_id);
    }

    let screening_id = Uuid::new_v4();
    sqlx::query(
        r#"INSERT INTO "ob-poc".screenings
           (screening_id, workstream_id, screening_type, status)
           VALUES ($1, $2, $3, 'PENDING')"#,
    )
    .bind(screening_id)
    .bind(workstream_id)
    .bind(screening_type)
    .execute(scope.executor())
    .await?;

    // Phase C.3 rollout (F7 follow-on, 2026-04-22): new screening
    // enters PENDING. Shared across screening.pep / .sanctions /
    // future .adverse-media because all of them enqueue through this
    // helper. The idempotent early-return at line 68 bypasses this
    // emission — existing PENDING screenings are NOT a state advance.
    let reason = format!(
        "screening.{} — new PENDING screening for workstream {}",
        screening_type.to_lowercase(),
        workstream_id
    );
    dsl_runtime::emit_pending_state_advance(
        ctx,
        screening_id,
        &format!("screening:{}:pending", screening_type.to_lowercase()),
        "screening/workstream",
        &reason,
    );

    ctx.bind("screening", screening_id);
    Ok(screening_id)
}

// ── screening.pep ─────────────────────────────────────────────────────────────

pub struct Pep;

#[async_trait]
impl SemOsVerbOp for Pep {
    fn fqn(&self) -> &str {
        "screening.pep"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let id = enqueue_workstream_screening(ctx, scope, entity_id, "PEP").await?;
        Ok(VerbExecutionOutcome::Uuid(id))
    }
}

// ── screening.sanctions ───────────────────────────────────────────────────────

pub struct Sanctions;

#[async_trait]
impl SemOsVerbOp for Sanctions {
    fn fqn(&self) -> &str {
        "screening.sanctions"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let entity_id = json_extract_uuid(args, ctx, "entity-id")?;
        let id = enqueue_workstream_screening(ctx, scope, entity_id, "SANCTIONS").await?;
        Ok(VerbExecutionOutcome::Uuid(id))
    }
}

// ── screening.adverse-media ───────────────────────────────────────────────────

pub struct AdverseMedia;

#[async_trait]
impl SemOsVerbOp for AdverseMedia {
    fn fqn(&self) -> &str {
        "screening.adverse-media"
    }
    async fn execute(
        &self,
        _args: &Value,
        _ctx: &mut VerbExecutionContext,
        _scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        Err(anyhow!("screening.adverse-media is not yet implemented"))
    }
}

// ── screening.bulk-refresh ────────────────────────────────────────────────────

pub struct BulkRefresh;

#[async_trait]
impl SemOsVerbOp for BulkRefresh {
    fn fqn(&self) -> &str {
        "screening.bulk-refresh"
    }
    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let case_id = json_extract_uuid(args, ctx, "case-id")?;
        let screening_type = json_extract_string_opt(args, "screening-type");

        let target_types: Vec<String> = match screening_type.as_deref() {
            Some(kind) => vec![kind.to_string()],
            None => vec![
                "SANCTIONS".to_string(),
                "PEP".to_string(),
                "ADVERSE_MEDIA".to_string(),
            ],
        };

        let workstream_ids: Vec<Uuid> = sqlx::query_scalar(
            r#"
            SELECT workstream_id
            FROM "ob-poc".entity_workstreams
            WHERE case_id = $1
            "#,
        )
        .bind(case_id)
        .fetch_all(scope.executor())
        .await?;

        let mut inserted = 0_u64;
        for workstream_id in workstream_ids {
            for st in &target_types {
                let screening_id = Uuid::new_v4();
                let result = sqlx::query(
                    r#"
                    INSERT INTO "ob-poc".screenings
                        (screening_id, workstream_id, screening_type, status)
                    SELECT $1, $2, $3, 'PENDING'
                    WHERE NOT EXISTS (
                        SELECT 1
                        FROM "ob-poc".screenings
                        WHERE workstream_id = $2 AND screening_type = $3
                    )
                    "#,
                )
                .bind(screening_id)
                .bind(workstream_id)
                .bind(st)
                .execute(scope.executor())
                .await?;
                inserted += result.rows_affected();
            }
        }

        Ok(VerbExecutionOutcome::Affected(inserted))
    }
}

// ── screening.await-aggregate-outcome ───────────────────────────────────────

/// Terminal `screenings.status` values a workstream's screening set must
/// all reach before this verb can compute a real answer. Matches the
/// `screening` DAG slot's own authoritative `terminal_states` (kyc_dag.yaml)
/// exactly — `ERROR`/`EXPIRED` are deliberately excluded: both loop back to
/// `PENDING` via `screening.bulk-refresh` (real transitions `ERROR ->
/// PENDING`, `EXPIRED -> PENDING`), i.e. they're recoverable, not terminal.
/// A screening sitting in `ERROR`/`EXPIRED` falls into the same "not all
/// siblings terminal yet" retry path as `PENDING`/`RUNNING`.
const TERMINAL_STATUSES: &[&str] = &["CLEAR", "HIT_CONFIRMED", "HIT_DISMISSED"];

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AwaitAggregateOutcomeResult {
    /// True if any sibling screening reached HIT_CONFIRMED.
    pub is_confirmed: bool,
    /// True if the worst outcome present is HIT_DISMISSED (no HIT_CONFIRMED
    /// among the siblings).
    pub is_dismissed: bool,
}

/// The `SlotTerminalStateAggregate` / `worst_of` reduction for the KYC
/// `entity_workstream.SCREEN → ASSESS/ENHANCED_DD` await
/// (EOP-PLAN-DAG-AWAITS-001 Phase 4/5): reads every `screenings` row for a
/// workstream (one per `screening_type` — SANCTIONS/PEP/ADVERSE_MEDIA/...),
/// and reduces them to the two boolean flags the compiled BPMN gateway
/// cascade reads. Runs as the `await_screening` task in
/// `config/bpmn/entity-workstream-screen.bpmn`, *after* the process's own
/// `screening_settled` message catch — by the time this executes, whatever
/// fired that signal (`ScreeningComplete`/`ScreeningReviewHit` in
/// `kyc_stream_ops.rs`) already confirmed every sibling was terminal. The
/// "not all siblings terminal yet" error below is a safety net for a race
/// (e.g. a new screening added between the signal firing and this running),
/// not the primary wait mechanism — `JobWorker`'s flat, non-backing-off
/// retry (see Phase 5 design addendum) can't absorb a multi-hour wait, so
/// the message catch does the actual waiting.
pub struct AwaitAggregateOutcome;

#[async_trait]
impl SemOsVerbOp for AwaitAggregateOutcome {
    fn fqn(&self) -> &str {
        "screening.await-aggregate-outcome"
    }

    async fn execute(
        &self,
        args: &Value,
        ctx: &mut VerbExecutionContext,
        scope: &mut dyn TransactionScope,
    ) -> Result<VerbExecutionOutcome> {
        let workstream_id = json_extract_uuid(args, ctx, "workstream-id")?;

        let statuses: Vec<String> = sqlx::query_scalar(
            r#"SELECT status FROM "ob-poc".screenings WHERE workstream_id = $1"#,
        )
        .bind(workstream_id)
        .fetch_all(scope.executor())
        .await?;

        let result = reduce_screening_statuses(workstream_id, &statuses)?;
        Ok(VerbExecutionOutcome::Record(serde_json::to_value(result)?))
    }
}

/// Pure worst-of reduction over a workstream's screening statuses — see
/// `AwaitAggregateOutcome`'s doc comment for the precedence rules.
fn reduce_screening_statuses(
    workstream_id: Uuid,
    statuses: &[String],
) -> Result<AwaitAggregateOutcomeResult> {
    if statuses.is_empty() {
        return Err(anyhow!(
            "No screenings found for workstream {workstream_id} — nothing to reduce"
        ));
    }

    if let Some(pending) = statuses.iter().find(|s| !TERMINAL_STATUSES.contains(&s.as_str())) {
        return Err(anyhow!(
            "Screening for workstream {workstream_id} still in progress (status '{pending}') \
             — not all siblings are terminal yet"
        ));
    }

    let is_confirmed = statuses.iter().any(|s| s == "HIT_CONFIRMED");
    let is_dismissed = !is_confirmed && statuses.iter().any(|s| s == "HIT_DISMISSED");

    Ok(AwaitAggregateOutcomeResult {
        is_confirmed,
        is_dismissed,
    })
}

#[cfg(test)]
mod await_aggregate_outcome_tests {
    use super::*;

    fn wid() -> Uuid {
        Uuid::nil()
    }

    fn statuses(strs: &[&str]) -> Vec<String> {
        strs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn all_clear_yields_clear() {
        let r = reduce_screening_statuses(wid(), &statuses(&["CLEAR", "CLEAR"])).unwrap();
        assert!(!r.is_confirmed);
        assert!(!r.is_dismissed);
    }

    #[test]
    fn any_hit_confirmed_wins_over_everything() {
        let r = reduce_screening_statuses(
            wid(),
            &statuses(&["CLEAR", "HIT_DISMISSED", "HIT_CONFIRMED"]),
        )
        .unwrap();
        assert!(r.is_confirmed);
        assert!(!r.is_dismissed);
    }

    #[test]
    fn error_status_is_not_terminal_yet() {
        // ERROR loops back to PENDING via screening.bulk-refresh — it is
        // not in the screening slot's terminal_states, so a sibling sitting
        // in ERROR means the set isn't ready to reduce yet.
        assert!(reduce_screening_statuses(wid(), &statuses(&["CLEAR", "ERROR"])).is_err());
    }

    #[test]
    fn expired_status_is_not_terminal_yet() {
        assert!(
            reduce_screening_statuses(wid(), &statuses(&["HIT_DISMISSED", "EXPIRED"])).is_err()
        );
    }

    #[test]
    fn hit_dismissed_wins_over_clear_when_no_confirmed() {
        let r = reduce_screening_statuses(wid(), &statuses(&["CLEAR", "HIT_DISMISSED"])).unwrap();
        assert!(!r.is_confirmed);
        assert!(r.is_dismissed);
    }

    #[test]
    fn empty_set_is_an_error() {
        assert!(reduce_screening_statuses(wid(), &[]).is_err());
    }

    #[test]
    fn non_terminal_status_is_an_error() {
        assert!(reduce_screening_statuses(wid(), &statuses(&["CLEAR", "PENDING"])).is_err());
        assert!(reduce_screening_statuses(wid(), &statuses(&["RUNNING"])).is_err());
        assert!(
            reduce_screening_statuses(wid(), &statuses(&["HIT_PENDING_REVIEW"])).is_err()
        );
    }
}
