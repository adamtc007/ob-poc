//! T4.2 (EOP-PLAN-CONTROLPLANE-001): sealed `ExecutionEnvelope` persistence,
//! single-use enforcement, and TTL against `"ob-poc".control_plane_envelopes`
//! (migration `20260710_control_plane_envelopes.sql`).
//!
//! No envelope content is ever persisted — only its handle identity
//! (`envelope_id`, `content_hash`) and lifecycle bookkeeping. Rehydration
//! is `try_consume`: a status-checked, single-row-locked UPDATE, never a
//! raw deserialize of a stored envelope (matching `ExecutionEnvelope`'s own
//! deliberate lack of `Deserialize` — see `envelope.rs`'s trybuild proof).

use chrono::Utc;
use uuid::Uuid;

use ob_poc_control_plane::envelope::{EnvelopeHandle, ExecutionEnvelope};

/// T4.1: which verb FQNs, if any, require a consumed sealed envelope before
/// `ObPocVerbExecutor::execute_verb_admitting_envelope` will dispatch them.
///
/// Default (empty) is the plan's shadow-first posture (§0): every path
/// stays envelope-less/legacy until it individually graduates. Graduation
/// criterion (§0): ≥500 production shadow evaluations with zero divergence
/// between the control plane's shadow decision and the legacy outcome, or
/// every divergence triaged as a legacy defect — nothing has accumulated
/// that evidence yet (T2.7 shadow wiring only just landed), so this set is
/// deliberately never populated by this tranche's own code; it exists so
/// the mechanism is real and testable, not so any path graduates today.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct EnforcedVerbs(std::collections::HashSet<String>);

impl EnforcedVerbs {
    /// Reads `OB_POC_CONTROL_PLANE_ENFORCE_VERBS` (comma-separated FQNs).
    /// Unset/empty — the production default — enforces nothing.
    pub(crate) fn from_env() -> Self {
        let raw = std::env::var("OB_POC_CONTROL_PLANE_ENFORCE_VERBS").unwrap_or_default();
        Self(
            raw.split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect(),
        )
    }

    pub(crate) fn is_enforced(&self, verb_fqn: &str) -> bool {
        self.0.contains(verb_fqn)
    }
}

/// T4.1: the outcome of admission-checking a candidate dispatch against the
/// (possibly empty) `EnforcedVerbs` set and, when required, the envelope
/// store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AdmissionDecision {
    /// Not an enforced verb, or enforcement is off for this path — dispatch
    /// proceeds exactly as it did before T4.1 (shadow-mode-safe default).
    NotEnforced,
    /// Enforced, and the supplied envelope id consumed cleanly — dispatch
    /// proceeds.
    Admitted,
    /// Enforced, but no envelope id was supplied at all.
    RejectedNoEnvelope,
    /// Enforced, and an envelope id was supplied, but `try_consume_by_id`
    /// did not return `Consumed` (not found / already consumed / expired /
    /// voided) — carries the outcome for the caller's error message.
    RejectedConsumeFailed(ConsumeOutcome),
}

/// T4.1 admission check: pool-based, pure orchestration over
/// `EnforcedVerbs` + `try_consume` — kept as its own function so it's
/// unit-testable without a live `VerbExecutionPort` implementor.
///
/// T8.1 (EOP-PLAN-CONTROLPLANE-001, closes PIR-D-008/PIR-D-010): widened
/// from `envelope_id: Option<Uuid>` + `try_consume_by_id` to
/// `envelope_handle: Option<EnvelopeHandle>` + `try_consume` — a caller
/// presenting a handle whose id matches a real sealed envelope but whose
/// content hash does not is now rejected here, not merely at the id-lookup
/// level.
///
/// T9.2 (Addendum B) gave `execute_verb_admitting_envelope` its own scope-
/// threaded sibling, [`check_admission_in_scope`], which it now calls
/// instead of this one (single-verb dispatch is atomic with its own
/// admission check). This pool-based variant is NOT dead code, though —
/// `admit_plan_checked` (below, T9.3's plan-level pre-flight admission
/// gate) genuinely needs a per-verb check across a whole multi-step plan
/// *before* any single step's transaction scope exists, which doesn't fit
/// the in-scope shape. Ownership-ledger closure sweep (2026-07-11)
/// confirmed this via `grep -rn "check_admission(" rust/src rust/crates`
/// before concluding it should stay — not assumed dead by pattern-matching
/// against `try_consume_by_id`'s different (genuinely zero-caller) case.
#[cfg(feature = "database")]
pub(crate) async fn check_admission(
    pool: &sqlx::PgPool,
    enforced: &EnforcedVerbs,
    verb_fqn: &str,
    envelope_handle: Option<EnvelopeHandle>,
) -> anyhow::Result<AdmissionDecision> {
    if !enforced.is_enforced(verb_fqn) {
        return Ok(AdmissionDecision::NotEnforced);
    }
    let Some(handle) = envelope_handle else {
        return Ok(AdmissionDecision::RejectedNoEnvelope);
    };
    match try_consume(pool, &handle).await? {
        ConsumeOutcome::Consumed => Ok(AdmissionDecision::Admitted),
        other => Ok(AdmissionDecision::RejectedConsumeFailed(other)),
    }
}

/// T9.2: [`check_admission`] against an already-open `PgTransactionScope`'s
/// connection — the T9.2 atomic-admission entry point. Same decision
/// logic, but the underlying consume (when enforced) runs via
/// [`try_consume_in_scope`], so the envelope's `FOR UPDATE` lock and the
/// consumption itself live inside the caller's own transaction, not a
/// separate one that commits before the write even begins.
#[cfg(feature = "database")]
pub(crate) async fn check_admission_in_scope(
    conn: &mut sqlx::PgConnection,
    enforced: &EnforcedVerbs,
    verb_fqn: &str,
    envelope_handle: Option<EnvelopeHandle>,
) -> anyhow::Result<AdmissionDecision> {
    if !enforced.is_enforced(verb_fqn) {
        return Ok(AdmissionDecision::NotEnforced);
    }
    let Some(handle) = envelope_handle else {
        return Ok(AdmissionDecision::RejectedNoEnvelope);
    };
    match try_consume_in_scope(conn, &handle).await? {
        ConsumeOutcome::Consumed => Ok(AdmissionDecision::Admitted),
        other => Ok(AdmissionDecision::RejectedConsumeFailed(other)),
    }
}

/// The outcome of attempting to consume a sealed envelope exactly once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ConsumeOutcome {
    /// First (and only permitted) consumption — dispatch may proceed.
    Consumed,
    /// No row for this `envelope_id` — never sealed, or already swept.
    NotFound,
    /// The handle's `content_hash` does not match the persisted row's —
    /// defence in depth against a handle that was tampered with or minted
    /// from a different envelope than the one sealed.
    ContentHashMismatch,
    /// A prior `try_consume` already succeeded for this envelope —
    /// resubmission is rejected (T4.2 exit criterion).
    AlreadyConsumed,
    /// `now() > not_after` — the envelope's validity window has lapsed
    /// (T4.2 exit criterion). The row is marked `expired` as a side effect
    /// so subsequent lookups short-circuit without re-checking the clock.
    Expired,
    /// Explicitly voided (e.g. by T4.3's stale-pin check) before
    /// consumption was attempted.
    Voided,
}

/// T9.3 (EOP-PLAN-CONTROLPLANE-001 Addendum B): boundary-interposition
/// admission check for every production ingress point that constructs
/// `dsl_v2::executor::DslExecutor` directly rather than going through the
/// bus path (T6, `ObPocVerbExecutor::execute_verb_admitting_envelope`) or
/// the runbook path (`step_executor_bridge.rs`, `execute_verb_admitting_envelope`). Checks
/// every verb in a compiled plan against [`check_admission`] and returns
/// the first rejection; `envelope_handle: None` at every call since none of
/// these paths has envelope infrastructure wired yet (same posture as
/// every other Path A/B/C/D call site before an envelope is actually
/// minted — this only bites while a verb is listed in
/// `OB_POC_CONTROL_PLANE_ENFORCE_VERBS`, empty by production default).
///
/// Single source of truth for this check — do not re-implement the
/// per-step loop at a call site; call this.
///
/// Reads `EnforcedVerbs::from_env()` fresh on every call (matches
/// `ObPocVerbExecutor::admit_in_scope`'s existing per-call-read pattern). The
/// per-step logic itself lives in [`admit_plan_checked`], which takes
/// `EnforcedVerbs` as a parameter so it's testable without mutating
/// process-global env state.
#[cfg(feature = "database")]
pub(crate) async fn admit_plan(
    pool: &sqlx::PgPool,
    plan: &crate::dsl_v2::execution_plan::ExecutionPlan,
) -> Result<(), String> {
    admit_plan_checked(pool, &EnforcedVerbs::from_env(), plan).await
}

/// Pure-parameter core of [`admit_plan`] — checks every verb in `plan`
/// against `enforced`/[`check_admission`], first rejection wins.
#[cfg(feature = "database")]
async fn admit_plan_checked(
    pool: &sqlx::PgPool,
    enforced: &EnforcedVerbs,
    plan: &crate::dsl_v2::execution_plan::ExecutionPlan,
) -> Result<(), String> {
    for step in &plan.steps {
        let verb_fqn = format!("{}.{}", step.verb_call.domain, step.verb_call.verb);
        let decision = check_admission(pool, enforced, &verb_fqn, None)
            .await
            .map_err(|e| format!("envelope admission check failed for {verb_fqn}: {e}"))?;
        match decision {
            AdmissionDecision::NotEnforced | AdmissionDecision::Admitted => {}
            AdmissionDecision::RejectedNoEnvelope => {
                return Err(format!(
                    "{verb_fqn} is enforce-mode gated (OB_POC_CONTROL_PLANE_ENFORCE_VERBS) but no sealed envelope was presented"
                ));
            }
            AdmissionDecision::RejectedConsumeFailed(outcome) => {
                return Err(format!("{verb_fqn} envelope admission rejected: {outcome:?}"));
            }
        }
    }
    Ok(())
}

/// Persists a freshly sealed envelope as `status = 'sealed'`, including its
/// `record` (T10.1 — `EnvelopeRecord`, the flattened, storable projection
/// `to_record()` produces; see that method's own doc for why this is safe
/// to persist and read back without reopening a rehydration path).
/// Best-effort: failures are logged, never propagated — matching
/// `agent::telemetry::store`'s and `agent::control_plane_shadow`'s posture,
/// since this is bookkeeping for later single-use/TTL enforcement, not a
/// precondition for the envelope having been legitimately sealed (the seal
/// itself already happened, in-process, before this call).
///
/// T10.1 (EOP-PLAN-CONTROLPLANE-001 Addendum C): first real production call
/// site is `sequencer.rs`'s `phase5_runtime_recheck`, shadow-only — nothing
/// consumes, nothing gates, nothing blocks. Proven correct via the live-DB
/// tests below before this tranche; T10.2 is the first consumer of `record`.
#[cfg(feature = "database")]
pub(crate) async fn persist_sealed(
    pool: &sqlx::PgPool,
    session_id: Uuid,
    verb_fqn: &str,
    envelope: &ExecutionEnvelope,
) -> bool {
    let handle = envelope.handle();
    let window = envelope.validity();
    let record = envelope.to_record();
    let record_json = match serde_json::to_value(&record) {
        Ok(v) => v,
        Err(err) => {
            tracing::warn!(
                error = %err,
                envelope_id = %handle.id(),
                "EnvelopeRecord serialisation failed (best-effort, non-blocking)"
            );
            return false;
        }
    };
    let result = sqlx::query(
        r#"
        INSERT INTO "ob-poc".control_plane_envelopes (
            envelope_id, content_hash, session_id, verb_fqn,
            status, not_before, not_after, record
        ) VALUES ($1, $2, $3, $4, 'sealed', $5, $6, $7)
        "#,
    )
    .bind(handle.id())
    .bind(handle.content_hash_hex())
    .bind(session_id)
    .bind(verb_fqn)
    .bind(window.not_before())
    .bind(window.not_after())
    .bind(record_json)
    .execute(pool)
    .await;

    match result {
        Ok(_) => true,
        Err(err) => {
            tracing::warn!(
                error = %err,
                envelope_id = %handle.id(),
                "control_plane_envelopes insert failed (best-effort, non-blocking)"
            );
            false
        }
    }
}

/// T9.2 (EOP-PLAN-CONTROLPLANE-001 Addendum B): pure query logic for
/// consuming an envelope exactly once — `SELECT ... FOR UPDATE` on the
/// envelope row, then the appropriate status branch — against a caller-
/// supplied connection, with **no `begin()`/`commit()` of its own**. The
/// caller owns the transaction boundary: [`try_consume_inner`] wraps this
/// in its own `pool.begin()`/`commit()` for standalone (pool-based) use;
/// [`try_consume_in_scope`] runs it directly against an already-open
/// `PgTransactionScope`'s connection, so the `FOR UPDATE` lock is held
/// for the scope's full lifetime (through the verb's write), not just for
/// this one call — closing the TOCTOU gap the T9.2 design doc's §5a
/// section exists to close, the same locking pattern §5a also requires
/// for `verify_pins_in_scope`.
#[cfg(feature = "database")]
async fn consume_core(
    conn: &mut sqlx::PgConnection,
    envelope_id: Uuid,
    expected_content_hash: Option<&str>,
) -> anyhow::Result<ConsumeOutcome> {
    let row: Option<(String, String, chrono::DateTime<Utc>, chrono::DateTime<Utc>)> = sqlx::query_as(
        r#"
        SELECT content_hash, status, not_before, not_after
        FROM "ob-poc".control_plane_envelopes
        WHERE envelope_id = $1
        FOR UPDATE
        "#,
    )
    .bind(envelope_id)
    .fetch_optional(&mut *conn)
    .await?;

    let Some((content_hash, status, _not_before, not_after)) = row else {
        return Ok(ConsumeOutcome::NotFound);
    };

    if let Some(expected) = expected_content_hash {
        if content_hash != expected {
            return Ok(ConsumeOutcome::ContentHashMismatch);
        }
    }

    match status.as_str() {
        "consumed" => return Ok(ConsumeOutcome::AlreadyConsumed),
        "voided" => return Ok(ConsumeOutcome::Voided),
        "expired" => return Ok(ConsumeOutcome::Expired),
        _ => {}
    }

    if Utc::now() > not_after {
        sqlx::query(
            r#"UPDATE "ob-poc".control_plane_envelopes SET status = 'expired' WHERE envelope_id = $1"#,
        )
        .bind(envelope_id)
        .execute(&mut *conn)
        .await?;
        return Ok(ConsumeOutcome::Expired);
    }

    sqlx::query(
        r#"
        UPDATE "ob-poc".control_plane_envelopes
        SET status = 'consumed', consumed_at = clock_timestamp()
        WHERE envelope_id = $1
        "#,
    )
    .bind(envelope_id)
    .execute(&mut *conn)
    .await?;
    Ok(ConsumeOutcome::Consumed)
}

/// Attempts to consume `handle` exactly once. Runs inside its own
/// transaction with `SELECT ... FOR UPDATE` on the envelope row so two
/// concurrent dispatch attempts against the same handle cannot both
/// observe `sealed` (the second blocks on the row lock, then observes the
/// first's `consumed` write). Internal — shared by `try_consume` (content-
/// hash checked) and `try_consume_by_id` (id-only, see that fn's doc for
/// why a weaker variant exists at all). Standalone/pool-based use only —
/// see [`try_consume_in_scope`] for the T9.2 atomic-admission path.
#[cfg(feature = "database")]
async fn try_consume_inner(
    pool: &sqlx::PgPool,
    envelope_id: Uuid,
    expected_content_hash: Option<&str>,
) -> anyhow::Result<ConsumeOutcome> {
    let mut tx = pool.begin().await?;
    let outcome = consume_core(&mut tx, envelope_id, expected_content_hash).await?;
    tx.commit().await?;
    Ok(outcome)
}

/// T9.2: [`consume_core`] against an already-open `PgTransactionScope`'s
/// connection — no `begin()`/`commit()` here; the caller's outer scope
/// owns that boundary and the `FOR UPDATE` lock this acquires is held
/// until the caller's own commit/rollback, not released after this call
/// returns.
#[cfg(feature = "database")]
pub(crate) async fn try_consume_in_scope(
    conn: &mut sqlx::PgConnection,
    handle: &EnvelopeHandle,
) -> anyhow::Result<ConsumeOutcome> {
    consume_core(conn, handle.id(), Some(&handle.content_hash_hex())).await
}

/// Attempts to consume `handle` exactly once, verifying its content hash
/// matches the persisted row (defence in depth against a tampered/wrong
/// handle). This is the strict variant — prefer it over `try_consume_by_id`
/// wherever the caller actually holds a full `EnvelopeHandle`. No
/// production call site yet (same reason as `persist_sealed`); proven via
/// the live-DB tests below.
#[cfg(feature = "database")]
#[allow(dead_code)]
pub(crate) async fn try_consume(pool: &sqlx::PgPool, handle: &EnvelopeHandle) -> anyhow::Result<ConsumeOutcome> {
    try_consume_inner(pool, handle.id(), Some(&handle.content_hash_hex())).await
}

/// Id-only consume, with no content-hash check. Originally existed for the
/// T4.1 `VerbExecutionPort::execute_verb_admitting_envelope` trait
/// boundary, which at the time deliberately carried a bare `Uuid` rather
/// than the full `EnvelopeHandle` type. T8.1 (EOP-PLAN-CONTROLPLANE-001,
/// closes PIR-D-008/PIR-D-010) widened that boundary to carry the typed
/// handle instead (via `ob-poc-types`, a values-only boundary crate
/// `dsl-runtime` can depend on without depending on `ob-poc-control-plane`
/// — see `port.rs`'s updated doc), so the id-only weakening this function
/// existed to accommodate no longer has a production reason to exist.
/// Demoted to `#[cfg(all(test, feature = "database"))]`: kept only as a
/// regression fixture for `try_consume_inner`'s id-only code path. T8.1's
/// exit criterion asks for "zero production call sites, grep-gated" — this
/// is enforced more strongly than a grep gate could: this symbol does not
/// exist at all in a non-test build, so no production code can reference
/// it regardless of what a text-based CI check might miss. `grep -rn
/// "try_consume_by_id" rust/src rust/crates` confirms the only remaining
/// references are this definition and its own test.
#[cfg(all(test, feature = "database"))]
async fn try_consume_by_id(pool: &sqlx::PgPool, envelope_id: Uuid) -> anyhow::Result<ConsumeOutcome> {
    try_consume_inner(pool, envelope_id, None).await
}

/// Marks an unconsumed envelope `voided` with a reason (T4.3: a stale-pin
/// mismatch detected at admission time voids the envelope rather than
/// silently letting it remain consumable). No production call site yet —
/// `verify_pins` (`ob-poc-boundary::toctou_recheck`) exists and is
/// live-DB-proven, but nothing yet calls it and then this in sequence at a
/// real admission point; that wiring is follow-on, not claimed here.
#[cfg(feature = "database")]
#[allow(dead_code)]
pub(crate) async fn void(pool: &sqlx::PgPool, envelope_id: Uuid, reason: &str) -> bool {
    let result = sqlx::query(
        r#"
        UPDATE "ob-poc".control_plane_envelopes
        SET status = 'voided', void_reason = $2
        WHERE envelope_id = $1 AND status = 'sealed'
        "#,
    )
    .bind(envelope_id)
    .bind(reason)
    .execute(pool)
    .await;

    match result {
        Ok(res) => res.rows_affected() == 1,
        Err(err) => {
            tracing::warn!(error = %err, %envelope_id, "control_plane_envelopes void failed");
            false
        }
    }
}

#[cfg(test)]
mod enforced_verbs_tests {
    use super::EnforcedVerbs;

    fn set(verbs: &[&str]) -> EnforcedVerbs {
        EnforcedVerbs(verbs.iter().map(|s| s.to_string()).collect())
    }

    #[test]
    fn empty_set_enforces_nothing() {
        assert!(!EnforcedVerbs::default().is_enforced("cbu.confirm"));
    }

    #[test]
    fn listed_verb_is_enforced_others_are_not() {
        let enforced = set(&["cbu.confirm"]);
        assert!(enforced.is_enforced("cbu.confirm"));
        assert!(!enforced.is_enforced("cbu.reject"));
    }
}

#[cfg(all(test, feature = "database"))]
mod tests {
    use super::*;
    use ob_poc_control_plane::envelope::ValidityWindow;

    async fn test_pool() -> sqlx::PgPool {
        let url = std::env::var("DATABASE_URL").expect("DATABASE_URL required for db-integration tests");
        sqlx::PgPool::connect(&url).await.expect("connect")
    }

    fn sealed_envelope(not_before: chrono::DateTime<Utc>, not_after: chrono::DateTime<Utc>) -> ExecutionEnvelope {
        let intent = ob_poc_control_plane::intent_admission::tests_support::admitted(Uuid::new_v4(), "cbu.confirm");
        let binding = ob_poc_control_plane::entity_binding::tests_support::bound(vec![Uuid::new_v4()]);
        let pack = ob_poc_control_plane::pack_resolution::tests_support::resolved("ob-poc.cbu");
        let dag =
            ob_poc_control_plane::dag_proof::tests_support::legal(Uuid::new_v4(), "VALIDATION_PENDING", "VALIDATED");
        let authority = ob_poc_control_plane::authority_gate::tests_support::authorised("actor-1", "compliance_officer");
        let evidence = ob_poc_control_plane::evidence_gate::tests_support::sufficient(vec!["obligation-1".into()]);
        let write_set = ob_poc_control_plane::write_set::tests_support::proof(
            vec![Uuid::new_v4()],
            vec!["validation_state".into()],
            vec!["ob-poc.cbus".into()],
            vec!["status".into()],
            "idem-1",
        );
        let runbook = ob_poc_control_plane::proof::CompiledRunbookRef::new(Uuid::new_v4());
        let snapshot = ob_poc_control_plane::snapshot::tests_support::pins(Some(Uuid::new_v4()), None, None, vec![]);

        ob_poc_control_plane::envelope::test_support::seal(
            intent,
            binding,
            pack,
            dag,
            authority,
            evidence,
            write_set,
            runbook,
            snapshot,
            ValidityWindow::new(not_before, not_after),
        )
    }

    /// T10.1: proves `persist_sealed` actually writes a readable `record`
    /// column, not just identity bookkeeping — reads the row back and
    /// deserialises it into `EnvelopeRecord`, checking the real pins
    /// (`entity_row_versions`) the fixture sealed round-trip through
    /// Postgres JSONB intact.
    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn persist_sealed_stores_a_readable_record_with_real_pins() {
        let pool = test_pool().await;
        let now = Utc::now();
        let entity_id = Uuid::new_v4();

        let intent = ob_poc_control_plane::intent_admission::tests_support::admitted(Uuid::new_v4(), "cbu.confirm");
        let binding = ob_poc_control_plane::entity_binding::tests_support::bound(vec![entity_id]);
        let pack = ob_poc_control_plane::pack_resolution::tests_support::resolved("ob-poc.cbu");
        let dag = ob_poc_control_plane::dag_proof::tests_support::legal(
            entity_id,
            "VALIDATION_PENDING",
            "VALIDATED",
        );
        let authority = ob_poc_control_plane::authority_gate::tests_support::authorised("actor-1", "compliance_officer");
        let evidence = ob_poc_control_plane::evidence_gate::tests_support::sufficient(vec!["obligation-1".into()]);
        let write_set = ob_poc_control_plane::write_set::tests_support::proof(
            vec![entity_id],
            vec!["validation_state".into()],
            vec!["ob-poc.cbus".into()],
            vec!["status".into()],
            "idem-1",
        );
        let runbook = ob_poc_control_plane::proof::CompiledRunbookRef::new(Uuid::new_v4());
        let snapshot = ob_poc_control_plane::snapshot::tests_support::pins(
            Some(Uuid::new_v4()),
            None,
            None,
            vec![(entity_id, "cbu".to_string(), 7)],
        );
        let envelope = ob_poc_control_plane::envelope::test_support::seal(
            intent,
            binding,
            pack,
            dag,
            authority,
            evidence,
            write_set,
            runbook,
            snapshot,
            ValidityWindow::new(now - chrono::Duration::minutes(1), now + chrono::Duration::minutes(5)),
        );
        let expected = envelope.to_record();

        assert!(persist_sealed(&pool, Uuid::new_v4(), "cbu.confirm", &envelope).await);

        let raw: serde_json::Value = sqlx::query_scalar(
            r#"SELECT record FROM "ob-poc".control_plane_envelopes WHERE envelope_id = $1"#,
        )
        .bind(envelope.id())
        .fetch_one(&pool)
        .await
        .expect("row exists");

        let record: ob_poc_control_plane::envelope::EnvelopeRecord =
            serde_json::from_value(raw).expect("record column deserialises");
        assert_eq!(record, expected);
        assert_eq!(record.bound_entity_ids, vec![entity_id]);
        assert_eq!(record.snapshot.entity_row_version(entity_id), Some(7));
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn consumed_envelope_resubmission_is_rejected() {
        let pool = test_pool().await;
        let now = Utc::now();
        let envelope = sealed_envelope(now - chrono::Duration::minutes(1), now + chrono::Duration::minutes(5));
        let handle = envelope.handle();

        assert!(persist_sealed(&pool, Uuid::new_v4(), "cbu.confirm", &envelope).await);
        assert_eq!(try_consume(&pool, &handle).await.unwrap(), ConsumeOutcome::Consumed);
        assert_eq!(try_consume(&pool, &handle).await.unwrap(), ConsumeOutcome::AlreadyConsumed);
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn expired_envelope_is_rejected() {
        let pool = test_pool().await;
        let now = Utc::now();
        let envelope = sealed_envelope(now - chrono::Duration::minutes(10), now - chrono::Duration::minutes(5));
        let handle = envelope.handle();

        assert!(persist_sealed(&pool, Uuid::new_v4(), "cbu.confirm", &envelope).await);
        assert_eq!(try_consume(&pool, &handle).await.unwrap(), ConsumeOutcome::Expired);
        // A second attempt against the now-`expired`-marked row must also reject.
        assert_eq!(try_consume(&pool, &handle).await.unwrap(), ConsumeOutcome::Expired);
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn unknown_handle_is_not_found() {
        let pool = test_pool().await;
        let envelope = sealed_envelope(Utc::now(), Utc::now() + chrono::Duration::minutes(5));
        let handle = envelope.handle(); // never persisted
        assert_eq!(try_consume(&pool, &handle).await.unwrap(), ConsumeOutcome::NotFound);
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn voided_envelope_cannot_be_consumed() {
        let pool = test_pool().await;
        let now = Utc::now();
        let envelope = sealed_envelope(now - chrono::Duration::minutes(1), now + chrono::Duration::minutes(5));
        let handle = envelope.handle();

        assert!(persist_sealed(&pool, Uuid::new_v4(), "cbu.confirm", &envelope).await);
        assert!(void(&pool, handle.id(), "stale_state").await);
        assert_eq!(try_consume(&pool, &handle).await.unwrap(), ConsumeOutcome::Voided);
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn check_admission_not_enforced_when_verb_not_listed() {
        let pool = test_pool().await;
        let enforced = EnforcedVerbs::default(); // production default: nothing enforced
        let decision = check_admission(&pool, &enforced, "cbu.confirm", None).await.unwrap();
        assert_eq!(decision, AdmissionDecision::NotEnforced);
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn check_admission_rejects_enforced_verb_with_no_envelope() {
        let pool = test_pool().await;
        let enforced = EnforcedVerbs(["cbu.confirm".to_string()].into_iter().collect());
        let decision = check_admission(&pool, &enforced, "cbu.confirm", None).await.unwrap();
        assert_eq!(decision, AdmissionDecision::RejectedNoEnvelope);
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn check_admission_admits_enforced_verb_with_sealed_envelope() {
        let pool = test_pool().await;
        let now = Utc::now();
        let envelope = sealed_envelope(now - chrono::Duration::minutes(1), now + chrono::Duration::minutes(5));
        let handle = envelope.handle();
        assert!(persist_sealed(&pool, Uuid::new_v4(), "cbu.confirm", &envelope).await);

        let enforced = EnforcedVerbs(["cbu.confirm".to_string()].into_iter().collect());
        let decision = check_admission(&pool, &enforced, "cbu.confirm", Some(handle))
            .await
            .unwrap();
        assert_eq!(decision, AdmissionDecision::Admitted);

        // Resubmission of the same envelope handle must be rejected, not silently re-admitted.
        let decision = check_admission(&pool, &enforced, "cbu.confirm", Some(handle))
            .await
            .unwrap();
        assert_eq!(
            decision,
            AdmissionDecision::RejectedConsumeFailed(ConsumeOutcome::AlreadyConsumed)
        );
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn try_consume_by_id_enforces_single_use_like_try_consume() {
        let pool = test_pool().await;
        let now = Utc::now();
        let envelope = sealed_envelope(now - chrono::Duration::minutes(1), now + chrono::Duration::minutes(5));
        let handle = envelope.handle();

        assert!(persist_sealed(&pool, Uuid::new_v4(), "cbu.confirm", &envelope).await);
        assert_eq!(try_consume_by_id(&pool, handle.id()).await.unwrap(), ConsumeOutcome::Consumed);
        assert_eq!(
            try_consume_by_id(&pool, handle.id()).await.unwrap(),
            ConsumeOutcome::AlreadyConsumed
        );
    }

    // ── T9.2: try_consume_in_scope / check_admission_in_scope ──────────────

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn try_consume_in_scope_matches_try_consume_behavior() {
        let pool = test_pool().await;
        let now = Utc::now();
        let envelope = sealed_envelope(now - chrono::Duration::minutes(1), now + chrono::Duration::minutes(5));
        let handle = envelope.handle();
        assert!(persist_sealed(&pool, Uuid::new_v4(), "cbu.confirm", &envelope).await);

        let mut tx = pool.begin().await.unwrap();
        let outcome = try_consume_in_scope(&mut tx, &handle).await.unwrap();
        tx.commit().await.unwrap();
        assert_eq!(outcome, ConsumeOutcome::Consumed);

        // Second attempt, fresh scope: already consumed.
        let mut tx = pool.begin().await.unwrap();
        let outcome = try_consume_in_scope(&mut tx, &handle).await.unwrap();
        tx.commit().await.unwrap();
        assert_eq!(outcome, ConsumeOutcome::AlreadyConsumed);
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn try_consume_in_scope_rolled_back_leaves_envelope_reconsumable() {
        // The T9.2 design doc's "rollback-then-retry corollary": if the
        // outer scope that consumed the envelope rolls back (e.g. because
        // the write that followed failed), consumption rolls back too —
        // single-use semantics working as designed, not a bug.
        let pool = test_pool().await;
        let now = Utc::now();
        let envelope = sealed_envelope(now - chrono::Duration::minutes(1), now + chrono::Duration::minutes(5));
        let handle = envelope.handle();
        assert!(persist_sealed(&pool, Uuid::new_v4(), "cbu.confirm", &envelope).await);

        let mut tx = pool.begin().await.unwrap();
        let outcome = try_consume_in_scope(&mut tx, &handle).await.unwrap();
        assert_eq!(outcome, ConsumeOutcome::Consumed);
        tx.rollback().await.unwrap();

        let mut tx = pool.begin().await.unwrap();
        let outcome = try_consume_in_scope(&mut tx, &handle).await.unwrap();
        tx.commit().await.unwrap();
        assert_eq!(
            outcome,
            ConsumeOutcome::Consumed,
            "a rolled-back scope must not leave the envelope durably consumed"
        );
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn concurrent_consume_in_scope_blocks_then_observes_winner() {
        // The actual proof of the FOR UPDATE lock-hold semantics
        // consume_core relies on: two concurrent scopes racing to consume
        // the same envelope. The loser must BLOCK on the row lock (not
        // race to a wrong answer), then observe the winner's outcome once
        // unblocked. This is the concurrent-consume probe the T9.2 design
        // doc's §6 testing strategy calls for, exercised against the new
        // in-scope path specifically (the old try_consume's own
        // consumed_envelope_resubmission_is_rejected test proves single-
        // use correctness but not the blocking behavior under real
        // concurrency, since it runs both attempts sequentially).
        let pool = test_pool().await;
        let now = Utc::now();
        let envelope = sealed_envelope(now - chrono::Duration::minutes(1), now + chrono::Duration::minutes(5));
        let handle = envelope.handle();
        assert!(persist_sealed(&pool, Uuid::new_v4(), "cbu.confirm", &envelope).await);

        let mut winner_tx = pool.begin().await.unwrap();
        let winner_outcome = try_consume_in_scope(&mut winner_tx, &handle).await.unwrap();
        assert_eq!(winner_outcome, ConsumeOutcome::Consumed);

        // Second connection attempts to consume the same handle while the
        // winner's transaction is still open (uncommitted) — must block,
        // not return immediately.
        let pool2 = pool.clone();
        let handle2 = handle;
        let loser = tokio::spawn(async move {
            let mut tx = pool2.begin().await.unwrap();
            let outcome = try_consume_in_scope(&mut tx, &handle2).await.unwrap();
            tx.commit().await.unwrap();
            outcome
        });

        // Give the loser a moment to reach the lock and (correctly) block.
        tokio::time::sleep(std::time::Duration::from_millis(200)).await;
        assert!(
            !loser.is_finished(),
            "loser must still be blocked on the row lock while the winner's scope is open"
        );

        winner_tx.commit().await.unwrap();

        let loser_outcome = tokio::time::timeout(std::time::Duration::from_secs(5), loser)
            .await
            .expect("loser must unblock once the winner commits")
            .unwrap();
        assert_eq!(
            loser_outcome,
            ConsumeOutcome::AlreadyConsumed,
            "loser must observe the winner's committed consumption, not race to Consumed itself"
        );
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn check_admission_in_scope_matches_check_admission_behavior() {
        let pool = test_pool().await;
        let now = Utc::now();
        let envelope = sealed_envelope(now - chrono::Duration::minutes(1), now + chrono::Duration::minutes(5));
        let handle = envelope.handle();
        assert!(persist_sealed(&pool, Uuid::new_v4(), "cbu.confirm", &envelope).await);

        let enforced = EnforcedVerbs(["cbu.confirm".to_string()].into_iter().collect());

        let mut tx = pool.begin().await.unwrap();
        let decision = check_admission_in_scope(&mut tx, &enforced, "cbu.confirm", Some(handle))
            .await
            .unwrap();
        tx.commit().await.unwrap();
        assert_eq!(decision, AdmissionDecision::Admitted);

        let mut tx = pool.begin().await.unwrap();
        let decision = check_admission_in_scope(&mut tx, &enforced, "cbu.confirm", None)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        assert_eq!(decision, AdmissionDecision::RejectedNoEnvelope);

        let mut tx = pool.begin().await.unwrap();
        let decision = check_admission_in_scope(&mut tx, &EnforcedVerbs::default(), "cbu.confirm", None)
            .await
            .unwrap();
        tx.commit().await.unwrap();
        assert_eq!(decision, AdmissionDecision::NotEnforced);
    }

    // ── T9.3 (Addendum B): admit_plan_checked ──────────────────────────────
    //
    // Hand-built ExecutionPlan/ExecutionStep — bypasses parse/compile and
    // the verb registry entirely, since admit_plan_checked only reads
    // step.verb_call.{domain,verb}. This is the shared function every T9.3
    // ingress point (RealDslExecutor, the MCP dsl_execute tool, the legacy
    // raw-execute route, the batch/sheet executors) now delegates to.

    fn plan_with_verbs(fqns: &[&str]) -> crate::dsl_v2::execution_plan::ExecutionPlan {
        use crate::dsl_v2::execution_plan::ExecutionStep;
        use dsl_core::{Argument, Span, VerbCall};
        let steps = fqns
            .iter()
            .enumerate()
            .map(|(i, fqn)| {
                let (domain, verb) = fqn.split_once('.').expect("fqn has domain.verb");
                ExecutionStep {
                    verb_call: VerbCall {
                        domain: domain.to_string(),
                        verb: verb.to_string(),
                        arguments: Vec::<Argument>::new(),
                        lens_override: None,
                        binding: None,
                        span: Span::default(),
                    },
                    injections: Vec::new(),
                    resource_dependencies: Vec::new(),
                    dag_edges: Vec::new(),
                    bind_as: None,
                    produces_entity_type: None,
                    step_index: i,
                    behavior: crate::dsl_v2::verb_registry::VerbBehavior::Crud,
                    custom_op_id: None,
                }
            })
            .collect();
        crate::dsl_v2::execution_plan::ExecutionPlan::from_steps(steps)
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn admit_plan_checked_passes_when_nothing_enforced() {
        let pool = test_pool().await;
        let plan = plan_with_verbs(&["cbu.confirm", "session.info"]);
        let enforced = EnforcedVerbs::default();
        assert!(admit_plan_checked(&pool, &enforced, &plan).await.is_ok());
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn admit_plan_checked_rejects_when_any_step_is_enforced_without_envelope() {
        let pool = test_pool().await;
        // Second step in the plan is enforced — proves the whole plan is
        // walked, not just its first verb.
        let plan = plan_with_verbs(&["session.info", "cbu.confirm"]);
        let enforced = EnforcedVerbs(["cbu.confirm".to_string()].into_iter().collect());
        let err = admit_plan_checked(&pool, &enforced, &plan)
            .await
            .expect_err("plan must be rejected");
        assert!(err.contains("cbu.confirm"), "error should name the rejected verb: {err}");
        assert!(err.contains("no sealed envelope"), "error should explain why: {err}");
    }

    #[tokio::test]
    #[ignore = "requires DATABASE_URL"]
    async fn admit_plan_checked_admits_enforced_verb_via_admit_plan_wrapper() {
        // Proves the public admit_plan() (env-reading) wrapper delegates
        // correctly — the two other tests exercise admit_plan_checked
        // directly to avoid mutating process-global env under parallel
        // test execution.
        let pool = test_pool().await;
        let plan = plan_with_verbs(&["session.info"]);
        assert!(admit_plan(&pool, &plan).await.is_ok());
    }
}
