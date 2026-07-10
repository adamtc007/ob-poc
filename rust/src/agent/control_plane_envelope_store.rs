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

/// T4.1 admission check: the single decision point
/// `execute_verb_admitting_envelope` overrides delegate to. Pure
/// orchestration over `EnforcedVerbs` + `try_consume_by_id` — kept as its
/// own function so it's unit-testable without a live `VerbExecutionPort`
/// implementor.
#[cfg(feature = "database")]
pub(crate) async fn check_admission(
    pool: &sqlx::PgPool,
    enforced: &EnforcedVerbs,
    verb_fqn: &str,
    envelope_id: Option<Uuid>,
) -> anyhow::Result<AdmissionDecision> {
    if !enforced.is_enforced(verb_fqn) {
        return Ok(AdmissionDecision::NotEnforced);
    }
    let Some(id) = envelope_id else {
        return Ok(AdmissionDecision::RejectedNoEnvelope);
    };
    match try_consume_by_id(pool, id).await? {
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

/// Persists a freshly sealed envelope as `status = 'sealed'`. Best-effort:
/// failures are logged, never propagated — matching
/// `agent::telemetry::store`'s and `agent::control_plane_shadow`'s posture,
/// since this is bookkeeping for later single-use/TTL enforcement, not a
/// precondition for the envelope having been legitimately sealed (the seal
/// itself already happened, in-process, before this call).
///
/// No production call site yet: nothing in `ob-poc` calls
/// `ExecutionEnvelope::seal()` today (that requires a full G1-G14
/// orchestration `evaluate()` this plan hasn't reached — G9/G10/G11/G12/G14
/// are still stubbed). Proven correct via the live-DB tests below; wiring a
/// real seal-producing call site is T4/T5 follow-on, not claimed here.
#[cfg(feature = "database")]
#[allow(dead_code)]
pub(crate) async fn persist_sealed(
    pool: &sqlx::PgPool,
    session_id: Uuid,
    verb_fqn: &str,
    envelope: &ExecutionEnvelope,
) -> bool {
    let handle = envelope.handle();
    let window = envelope.validity();
    let result = sqlx::query(
        r#"
        INSERT INTO "ob-poc".control_plane_envelopes (
            envelope_id, content_hash, session_id, verb_fqn,
            status, not_before, not_after
        ) VALUES ($1, $2, $3, $4, 'sealed', $5, $6)
        "#,
    )
    .bind(handle.id())
    .bind(handle.content_hash_hex())
    .bind(session_id)
    .bind(verb_fqn)
    .bind(window.not_before())
    .bind(window.not_after())
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

/// Attempts to consume `handle` exactly once. Runs inside its own
/// transaction with `SELECT ... FOR UPDATE` on the envelope row so two
/// concurrent dispatch attempts against the same handle cannot both
/// observe `sealed` (the second blocks on the row lock, then observes the
/// first's `consumed` write). Internal — shared by `try_consume` (content-
/// hash checked) and `try_consume_by_id` (id-only, see that fn's doc for
/// why a weaker variant exists at all).
#[cfg(feature = "database")]
async fn try_consume_inner(
    pool: &sqlx::PgPool,
    envelope_id: Uuid,
    expected_content_hash: Option<&str>,
) -> anyhow::Result<ConsumeOutcome> {
    let mut tx = pool.begin().await?;

    let row: Option<(String, String, chrono::DateTime<Utc>, chrono::DateTime<Utc>)> = sqlx::query_as(
        r#"
        SELECT content_hash, status, not_before, not_after
        FROM "ob-poc".control_plane_envelopes
        WHERE envelope_id = $1
        FOR UPDATE
        "#,
    )
    .bind(envelope_id)
    .fetch_optional(&mut *tx)
    .await?;

    let Some((content_hash, status, _not_before, not_after)) = row else {
        tx.commit().await?;
        return Ok(ConsumeOutcome::NotFound);
    };

    if let Some(expected) = expected_content_hash {
        if content_hash != expected {
            tx.commit().await?;
            return Ok(ConsumeOutcome::ContentHashMismatch);
        }
    }

    match status.as_str() {
        "consumed" => {
            tx.commit().await?;
            return Ok(ConsumeOutcome::AlreadyConsumed);
        }
        "voided" => {
            tx.commit().await?;
            return Ok(ConsumeOutcome::Voided);
        }
        "expired" => {
            tx.commit().await?;
            return Ok(ConsumeOutcome::Expired);
        }
        _ => {}
    }

    if Utc::now() > not_after {
        sqlx::query(
            r#"UPDATE "ob-poc".control_plane_envelopes SET status = 'expired' WHERE envelope_id = $1"#,
        )
        .bind(envelope_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
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
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(ConsumeOutcome::Consumed)
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

/// Id-only consume, with no content-hash check. Exists solely for the T4.1
/// `VerbExecutionPort::execute_verb_admitting_envelope` trait boundary,
/// which deliberately carries a bare `Uuid` rather than the full
/// `EnvelopeHandle` type (so `dsl-runtime`, the pure execution-tier
/// contract crate, does not need a dependency on `ob-poc-control-plane` —
/// see that trait method's doc). This is a real, acknowledged weakening
/// versus `try_consume`: a caller with only the id (not the content hash)
/// cannot detect a handle minted from a different envelope. Strengthening
/// this requires threading a typed handle through the dispatch boundary,
/// which is follow-on work (not yet done — see the ownership ledger), not
/// silently claimed here.
#[cfg(feature = "database")]
pub(crate) async fn try_consume_by_id(pool: &sqlx::PgPool, envelope_id: Uuid) -> anyhow::Result<ConsumeOutcome> {
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
        let decision = check_admission(&pool, &enforced, "cbu.confirm", Some(handle.id()))
            .await
            .unwrap();
        assert_eq!(decision, AdmissionDecision::Admitted);

        // Resubmission of the same envelope id must be rejected, not silently re-admitted.
        let decision = check_admission(&pool, &enforced, "cbu.confirm", Some(handle.id()))
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
}
