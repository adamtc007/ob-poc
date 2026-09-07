//! 100% live-DB integration coverage for the dsl.kyc verb surface
//! (kyc.role.assign/withdraw retired 2026-08-12, T0.3 K-G7 fold-blind write;
//! `kyc_ubo.assert.obligation.creation`/`.satisfaction` DISSOLVED and
//! `.waiver` MOVED to `kyc_ubo.decide.obligation.waiver`, D2.0 §5,
//! 2026-08-22).
//!
//! Each verb is exercised via its `SemOsVerbOp` through a real
//! `VerbExecutionContext` and `TransactionScope`. The test commits to the
//! durable stream and asserts that the event lands with the correct `verb_fqn`.
//! Verbs with preconditions run in natural dependency order
//! (assert → attach-evidence → verify, etc.).
//!
//! Verbs already proven in dedicated test files:
//!   kyc_ubo.assert.edge.connect          → tests/kyc_stream_ops.rs
//!   kyc_ubo.assert.subject.place                 → tests/kyc_stream_ops.rs
//!
//! `coverage_kyc_obligation_update_identity/screening/risk` below now assert
//! REFUSAL, not success — the disclosed D2.0 P0 consequence: with `creation`
//! dissolved (the only writer of a new `ObligationTracks` entry),
//! `Precondition::ObligationExists` can never again be satisfied for these
//! three verbs. Recorded, not silently left to bit-rot as a mystery failure.

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::TransactionScope;
use ob_poc::domain_ops::kyc_stream_ops::{
    KycObligationUpdateIdentity, KycObligationUpdateRisk, KycObligationUpdateScreening,
    KycSubjectPlace, KycSubjectRecordEnquiry, UboDeterminationFreeze, UboEdgeAttachEvidence,
    UboEdgeConnect, UboEdgeDisconnect, UboEdgeRetract,
};
// kyc.person.approve/.reject renamed kyc_ubo.decide.subject.approve/.reject TS.6 P2 — moved
// to ob-poc-kyc-decide. kyc_ubo.decide.obligation.waiver moved there too, D2.0 §5.
use ob_poc_kyc_decide::{DecideApprove, DecideObligationWaive, DecideReject};
use ob_poc_kyc_substrate::SubjectId;
use ob_poc_types::TransactionScopeId;
use sem_os_postgres::ops::SemOsVerbOp;

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string())
}

async fn pool() -> PgPool {
    PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url())
        .await
        .expect("connect to test DB")
}

struct Scope {
    tx: Transaction<'static, Postgres>,
    pool: PgPool,
    id: TransactionScopeId,
}
impl Scope {
    async fn begin(p: &PgPool) -> Self {
        Self {
            tx: p.begin().await.unwrap(),
            pool: p.clone(),
            id: TransactionScopeId::new(),
        }
    }
    async fn commit(self) {
        self.tx.commit().await.unwrap();
    }
}
impl TransactionScope for Scope {
    fn scope_id(&self) -> TransactionScopeId {
        self.id
    }
    fn transaction(&mut self) -> &mut Transaction<'static, Postgres> {
        &mut self.tx
    }
    fn pool(&self) -> &PgPool {
        &self.pool
    }
}

/// Dispatch a verb op and commit. Returns the raw VerbExecutionOutcome as
/// JSON. Runs under a real session identity (D2.1 §7 Q3) via the
/// `test-fixtures`-gated `test_verb_execution_context_with_session` — the
/// `decide.*` ops in this file refuse a context with no session_id.
async fn run(op: &dyn SemOsVerbOp, args: serde_json::Value, pool: &PgPool) -> serde_json::Value {
    let mut ctx = ob_poc_kyc_decide::test_verb_execution_context_with_session(Uuid::new_v4());
    let mut scope = Scope::begin(pool).await;
    let out = op
        .execute(&args, &mut ctx, &mut scope)
        .await
        .unwrap_or_else(|error| panic!("{}: {error}", op.fqn()));
    scope.commit().await;
    serde_json::to_value(format!("{:?}", out)).unwrap()
}

/// §8 Q3: `connect` refuses a caller-supplied edge id — it mints one and
/// returns it. This file's `run()` Debug-stringifies the whole outcome, so
/// this dedicated helper calls the op directly and extracts the real
/// `edge_id` for tests that need to reference a KNOWN edge afterward
/// (attach-evidence/verify/disconnect setup).
async fn run_connect(
    subject: SubjectId,
    from: Uuid,
    to: Uuid,
    kind: &str,
    pool: &PgPool,
) -> Uuid {
    let mut ctx = ob_poc_kyc_decide::test_verb_execution_context_with_session(Uuid::new_v4());
    let mut scope = Scope::begin(pool).await;
    let out = UboEdgeConnect
        .execute(
            &serde_json::json!({
                "subject-id": subject.0, "from_entity_id": from, "to_entity_id": to, "kind": kind,
            }),
            &mut ctx,
            &mut scope,
        )
        .await
        .expect("connect must succeed");
    scope.commit().await;
    let dsl_runtime::VerbExecutionOutcome::Record(v) = out else {
        panic!("connect must return a Record outcome, got {out:?}");
    };
    Uuid::parse_str(v["edge_id"].as_str().expect("edge_id")).expect("edge_id must be a valid UUID")
}

/// EOP-DD-UBO-PROOF-001 §4 (T5): `evidence` returns the logged proof's own
/// citation id — the fact `retract` later targets. Mirrors `run_connect`'s
/// direct-call-and-extract shape.
async fn run_attach_evidence(subject: SubjectId, edge: Uuid, pool: &PgPool) -> Uuid {
    let mut ctx = ob_poc_kyc_decide::test_verb_execution_context_with_session(Uuid::new_v4());
    let mut scope = Scope::begin(pool).await;
    let out = UboEdgeAttachEvidence
        .execute(
            &serde_json::json!({
                "subject-id": subject.0, "edge-id": edge,
                "kind": "filed-document", "source": "test fixture", "date": "2026-08-28",
            }),
            &mut ctx,
            &mut scope,
        )
        .await
        .expect("attach-evidence must succeed");
    scope.commit().await;
    let dsl_runtime::VerbExecutionOutcome::Record(v) = out else {
        panic!("attach-evidence must return a Record outcome, got {out:?}");
    };
    Uuid::parse_str(v["citation_id"].as_str().expect("citation_id"))
        .expect("citation_id must be a valid UUID")
}

/// Assert `verb_fqn` appears in kyc_intent_events for subject.
async fn assert_event(pool: &PgPool, subject: SubjectId, verb_fqn: &str) {
    let count: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM "ob-poc".kyc_intent_events
           WHERE subject_root = $1 AND verb_fqn = $2"#,
    )
    .bind(subject.0)
    .bind(verb_fqn)
    .fetch_one(pool)
    .await
    .unwrap();
    assert!(
        count > 0,
        "expected event {verb_fqn} for subject {}",
        subject.0
    );
}

/// kyc_ubo.decide.subject.approve/kyc_ubo.decide.subject.reject write to `kyc_decision_records`, not the
/// fact stream (TS.6 P2) — this is `assert_event`'s counterpart for them.
async fn assert_decision_record(pool: &PgPool, subject: SubjectId, verb_fqn: &str) {
    let count: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM "ob-poc".kyc_decision_records
           WHERE subject_root = $1 AND verb_fqn = $2"#,
    )
    .bind(subject.0)
    .bind(verb_fqn)
    .fetch_one(pool)
    .await
    .unwrap();
    assert!(
        count > 0,
        "expected decision record {verb_fqn} for subject {}",
        subject.0
    );
}

async fn cleanup(pool: &PgPool, subjects: &[SubjectId]) {
    for s in subjects {
        for t in [
            "kyc_intent_events",
            "kyc_subject_streams",
            "kyc_control_edge_projection",
            // kyc_ubo.decide.subject.approve/kyc_ubo.decide.subject.reject write here now, not the fact
            // stream (TS.6 P2) — cleaned up alongside the other tables.
            "kyc_decision_records",
            // D2.0 §4 run book — same cleanup story.
            "kyc_evaluation_runs",
        ] {
            let _ = sqlx::query(&format!(
                r#"DELETE FROM "ob-poc".{t} WHERE subject_root = $1"#
            ))
            .bind(s.0)
            .execute(pool)
            .await;
        }
        let _ = sqlx::query(
            r#"DELETE FROM "public".outbox WHERE idempotency_key LIKE $1
               OR (payload->>'determination_subject')::text = $2
               OR (payload->>'subject_root')::text = $2"#,
        )
        .bind(format!("{}:%", s.0))
        .bind(s.0.to_string())
        .execute(pool)
        .await;
    }
}

// ── Edge lifecycle (ubo.edge.*) ────────────────────────────────────────────────

// EOP-VS-UBO-GAME-001 T3 (§3.2, 2026-08-27): `assert-control` +
// `assert-economic-interest` MERGED into `connect` — one coverage test,
// exercised once with `kind: economic_interest` (the sub-case that also
// carries `percentage`); `coverage_ubo_edge_attach_evidence`/`_verify`/
// `_disconnect` below each independently prove `connect` with
// `kind: voting_rights` as their setup step.
#[tokio::test]
async fn coverage_ubo_edge_connect() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    run(
        &KycSubjectPlace,
        serde_json::json!({ "subject-id": subject.0, "entity-type": "private_limited_company" }),
        &pool,
    )
    .await;
    run(
        &UboEdgeConnect,
        serde_json::json!({
            "subject-id": subject.0,
            "from_entity_id": Uuid::new_v4(),
            "to_entity_id": Uuid::new_v4(),
            "kind": "economic_interest",
            "percentage": 45.0,
        }),
        &pool,
    )
    .await;
    assert_event(&pool, subject, "kyc_ubo.assert.edge.connect").await;
    cleanup(&pool, &[subject]).await;
}

#[tokio::test]
async fn coverage_ubo_edge_attach_evidence() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    run(
        &KycSubjectPlace,
        serde_json::json!({ "subject-id": subject.0, "entity-type": "private_limited_company" }),
        &pool,
    )
    .await;
    let edge = run_connect(subject, Uuid::new_v4(), Uuid::new_v4(), "voting_rights", &pool).await;
    run(
        &UboEdgeAttachEvidence,
        serde_json::json!({
            "subject-id": subject.0, "edge-id": edge,
            "kind": "filed-document", "source": "test fixture", "date": "2026-08-28",
        }),
        &pool,
    )
    .await;
    assert_event(&pool, subject, "kyc_ubo.assert.edge.evidence").await;
    cleanup(&pool, &[subject]).await;
}

// `coverage_ubo_edge_verify` RETIRED (EOP-DD-UBO-PROOF-001 §3/§4, T5,
// 2026-08-28) alongside `kyc_ubo.assert.edge.verification`/`UboEdgeVerify`
// (K-G7: 0 real committed events under that FQN) — no ratchet left to
// exercise. `coverage_ubo_edge_retract`, below, is T5's replacement
// coverage: `UboEdgeRetract` withdraws the proof `UboEdgeAttachEvidence`
// just logged.
#[tokio::test]
async fn coverage_ubo_edge_retract() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    run(
        &KycSubjectPlace,
        serde_json::json!({ "subject-id": subject.0, "entity-type": "private_limited_company" }),
        &pool,
    )
    .await;
    let edge = run_connect(subject, Uuid::new_v4(), Uuid::new_v4(), "voting_rights", &pool).await;
    let citation = run_attach_evidence(subject, edge, &pool).await;
    run(
        &UboEdgeRetract,
        serde_json::json!({ "subject-id": subject.0, "citation-id": citation }),
        &pool,
    )
    .await;
    assert_event(&pool, subject, "kyc_ubo.assert.edge.retract").await;
    cleanup(&pool, &[subject]).await;
}

#[tokio::test]
async fn coverage_ubo_edge_disconnect() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    run(
        &KycSubjectPlace,
        serde_json::json!({ "subject-id": subject.0, "entity-type": "private_limited_company" }),
        &pool,
    )
    .await;
    let edge = run_connect(subject, Uuid::new_v4(), Uuid::new_v4(), "voting_rights", &pool).await;
    run(
        &UboEdgeDisconnect,
        serde_json::json!({ "subject-id": subject.0, "edge-id": edge }),
        &pool,
    )
    .await;
    assert_event(&pool, subject, "kyc_ubo.assert.edge.disconnect").await;
    // K-13: edge still in stream, not deleted
    let edge_count: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM "ob-poc".kyc_intent_events WHERE subject_root = $1 AND verb_fqn = 'kyc_ubo.assert.edge.connect'"#,
    ).bind(subject.0).fetch_one(&pool).await.unwrap();
    assert_eq!(
        edge_count, 1,
        "K-13: connect event stays in stream after disconnect"
    );
    cleanup(&pool, &[subject]).await;
}

// `coverage_ubo_edge_reconcile_conflict` RETIRED (EOP-VS-UBO-GAME-001 T3,
// §3.3, 2026-08-27, K-G7): `kyc_ubo.assert.edge.reconciliation` no longer
// exists — "No reconcile ... a third path to what two moves already do."

// ── Determination verbs (ubo.determination.*) ─────────────────────────────────
//
// `coverage_ubo_determination_select_strategy` RETIRED (TS.6 P2, K-G7):
// `ubo.determination.select-strategy` no longer exists — the strategy is
// derived from `structure_class` (`strategy_for_structure_class`), never
// separately asserted.
//
// `coverage_ubo_determination_compute_fold` RETIRED (TS.6 P2, K-G7):
// `ubo.determination.compute-fold` no longer exists — it was a derivation
// dressed as a verb, carrying the identical [ReconciledProjection,
// StructureClassSupported] precondition pair `freeze` already independently
// declares. Nothing needed building to preserve the gate elsewhere; see
// `coverage_ubo_determination_freeze` below and `coverage_ubo_determination_
// apply_smo_fallback`'s own precondition coverage for the surviving proof.

// `coverage_ubo_determination_apply_smo_fallback` RETIRED (TS.6 §5,
// 2026-08-22), mirroring the select-strategy/compute-fold retirements: the
// verb is gone, so there is no live op to cover. SMO now reaches a
// determination only via the traversal's pull-on-exhaustion (TS.3 §4a),
// which is covered by the determination tests, not by a verb-coverage row.

#[tokio::test]
async fn coverage_ubo_determination_freeze() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let owner = Uuid::new_v4();
    // EOP-DD-UBO-DISPATCH-001 T4-close (2026-08-28): `place`'s own
    // entity-type already drives dispatch (`ownership_prong_strategy` for
    // `private_limited_company`) — the separate `structure-class` call was
    // pure setup, redundant with what `place` already asserted.
    run(
        &KycSubjectPlace,
        serde_json::json!({ "subject-id": subject.0, "entity-type": "private_limited_company" }),
        &pool,
    )
    .await;
    // K-5: a determination must never be silent. This used to be satisfied by
    // asserting an SMO (`ubo.determination.apply-smo-fallback`, retired
    // TS.6 §5 — SMO is PULLED on exhaustion by the traversal, never written).
    // With no manual override left, freeze needs a REAL candidate: a natural
    // person holding ≥ the 25% default threshold directly in the subject.
    run(
        &KycSubjectPlace,
        serde_json::json!({
            "subject-id": subject.0, "entity-id": owner, "entity-type": "natural_person",
        }),
        &pool,
    )
    .await;
    run(
        &UboEdgeConnect,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": owner, "to_entity_id": subject.0,
            "kind": "economic_interest", "percentage": 60.0,
        }),
        &pool,
    )
    .await;
    run(
        &UboDeterminationFreeze,
        serde_json::json!({
            "subject-id": subject.0, "policy-version": "v1.0",
        }),
        &pool,
    )
    .await;
    assert_event(&pool, subject, "kyc_ubo.decide.determination.freeze").await;
    cleanup(&pool, &[subject]).await;
}

// ── Subject taxonomy (kyc.subject.*) ─────────────────────────────────────────

// `coverage_kyc_subject_classify_structure` REWRITTEN (EOP-DD-UBO-DISPATCH-001
// T4-close, 2026-08-28) — `kyc_ubo.assert.subject.structure-class` is
// retired; a coverage row proving a deleted verb's event gets recorded has
// no mechanism left to guard ("a proof cannot outlive the mechanism it
// guards," the same reasoning `kyc_t63_studs.rs` row 10 was superseded
// under). `place`'s own event coverage already lives in
// `kyc_t2_place_remove.rs::place_records_entity_and_type_in_one_move`, so
// rather than duplicate it here, this row is repurposed onto `enquiry` —
// the one other subject-taxonomy verb this file never covered — keeping
// the row's original job (prove a subject-taxonomy verb's event is real)
// against a verb that still exists.
#[tokio::test]
async fn coverage_kyc_subject_enquiry() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    run(
        &KycSubjectPlace,
        serde_json::json!({ "subject-id": subject.0, "entity-type": "private_limited_company" }),
        &pool,
    )
    .await;
    run(
        &KycSubjectRecordEnquiry,
        serde_json::json!({
            "subject-id": subject.0,
            "sources-consulted": ["GLEIF"],
            "searches-run": ["company registry search"],
        }),
        &pool,
    )
    .await;
    assert_event(&pool, subject, "kyc_ubo.assert.subject.enquiry").await;
    cleanup(&pool, &[subject]).await;
}

// ── Role-basis (kyc.role.*) ───────────────────────────────────────────────────

// coverage_kyc_role_withdraw removed 2026-08-12 — kyc.role.assign/withdraw
// retired (T0.3 K-G7 fold-blind write); see dsl-kyc-obligation.yaml's
// retirement comment.

// ── Obligation lifecycle (kyc.obligation.*) ───────────────────────────────────

/// D2.0 P0 first found this refused (`creation`, the only writer of a new
/// `ObligationTracks` entry, was dissolved, so `Precondition::ObligationExists`
/// could never again be satisfied). EOP-DD-UBO-CLEANOUT-001 T6 P2
/// (2026-09-07) went further: `fold/obligation.rs` — and with it
/// `Precondition::ObligationExists` itself — is deleted entirely, so there
/// is no longer a precondition left to refuse on. This verb was already not
/// precondition-checked at the op layer either way (`validate_entry_fqn:
/// None`, `kyc_stream_ops.rs`, T6.4 rows 12-14) — it now succeeds
/// unconditionally, appending a genuinely fold-blind event: no fold arm
/// ever reads it back (`kyc_pack_closure.rs::fold_blind_verbs_are_exactly_known`
/// allow-lists exactly this verb for that reason).
#[tokio::test]
async fn coverage_kyc_obligation_update_identity() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    run(
        &KycSubjectPlace,
        serde_json::json!({ "subject-id": subject.0, "entity-type": "natural_person" }),
        &pool,
    )
    .await;
    run(
        &KycObligationUpdateIdentity,
        serde_json::json!({
            "subject-id": subject.0, "obligation-id": Uuid::new_v4(), "state": "satisfied",
        }),
        &pool,
    )
    .await;
    assert_event(&pool, subject, "kyc_ubo.assert.entity.identity").await;
    cleanup(&pool, &[subject]).await;
}

/// Same disclosed consequence as `update-identity`, above.
#[tokio::test]
async fn coverage_kyc_obligation_update_screening() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    run(
        &KycSubjectPlace,
        serde_json::json!({ "subject-id": subject.0, "entity-type": "natural_person" }),
        &pool,
    )
    .await;
    run(
        &KycObligationUpdateScreening,
        serde_json::json!({
            "subject-id": subject.0, "obligation-id": Uuid::new_v4(), "state": "satisfied",
        }),
        &pool,
    )
    .await;
    assert_event(&pool, subject, "kyc_ubo.assert.entity.screening").await;
    cleanup(&pool, &[subject]).await;
}

/// Same disclosed consequence as `update-identity`, above.
#[tokio::test]
async fn coverage_kyc_obligation_update_risk() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    run(
        &KycSubjectPlace,
        serde_json::json!({ "subject-id": subject.0, "entity-type": "natural_person" }),
        &pool,
    )
    .await;
    run(
        &KycObligationUpdateRisk,
        serde_json::json!({
            "subject-id": subject.0, "obligation-id": Uuid::new_v4(), "state": "in_progress",
        }),
        &pool,
    )
    .await;
    assert_event(&pool, subject, "kyc_ubo.assert.entity.risk").await;
    cleanup(&pool, &[subject]).await;
}

/// `kyc_ubo.decide.obligation.waiver` (moved from `kyc_ubo.assert.obligation.waiver`,
/// D2.0 §5) — writes only to `kyc_decision_records`, citing the run, never
/// the fact stream.
#[tokio::test]
async fn coverage_kyc_decide_obligation_waiver() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    run(
        &KycSubjectPlace,
        serde_json::json!({ "subject-id": subject.0, "entity-type": "private_limited_company" }),
        &pool,
    )
    .await;
    run(
        &DecideObligationWaive,
        serde_json::json!({
            "subject-id": subject.0,
            "check-id": "board.every-entity-has-a-proven-type",
            "reason": "entity is regulated financial institution — simplified due diligence applies",
        }),
        &pool,
    )
    .await;
    assert_decision_record(&pool, subject, "kyc_ubo.decide.obligation.waiver").await;
    cleanup(&pool, &[subject]).await;
}

// ── Person decision (kyc.person.*) ────────────────────────────────────────────

#[tokio::test]
async fn coverage_kyc_person_reject() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    run(
        &KycSubjectPlace,
        serde_json::json!({ "subject-id": subject.0, "entity-type": "natural_person" }),
        &pool,
    )
    .await;
    run(
        &DecideReject,
        serde_json::json!({
            "subject-id": subject.0,
            "reason": "sanctions match confirmed — PEP designation upheld",
        }),
        &pool,
    )
    .await;
    assert_decision_record(&pool, subject, "kyc_ubo.decide.subject.reject").await;
    cleanup(&pool, &[subject]).await;
}

/// D2.0 §6 `decide_cites_a_run` — replaces TS.6 §8's
/// `decide_verbs_cite_their_basis` (which cited an obligation-fold snapshot,
/// meaningless once `creation` dissolved). Every verdict now cites the
/// `EvaluationRun` it relied on: `basis.run_id` names a row in
/// `kyc_evaluation_runs`, and the cited run's `board_state_hash` matches
/// what `run_id` row actually recorded — not just present, but genuinely
/// the run that ran. Exercises both landed verdicts: `kyc_ubo.decide.subject.approve`
/// (against a genuinely EMPTY board, vacuously satisfying `ProvenTypeCheck`,
/// D2.1 §2, so K-23 does not refuse this run — deliberately the SIMPLEST
/// passing case, not a claim that a real Proved board is unreachable:
/// `UboEdgeAttachEvidence` called with `entity-id` instead of `edge-id`
/// evidences an entity's type instead of an edge, logging a real cited
/// proof (EOP-DD-UBO-PROOF-001 §4) — wired 2026-08-24 corrective tranche
/// Item 3a, exercised by `tests/kyc_d21_engine.rs`'s
/// `unevaluable_flips_to_pass_through_production`, not duplicated here)
/// and `kyc_ubo.decide.subject.reject` (citation recorded even though
/// rejection is allowed at any stage, board has one registered-but-untyped
/// entity so `ProvenTypeCheck` returns `Unevaluable` — irrelevant to
/// reject, which never gates on the work list).
#[tokio::test]
async fn decide_cites_a_run() {
    let pool = pool().await;

    for (subject, op, fqn, extra_args, register_first) in [
        (
            SubjectId(Uuid::new_v4()),
            &DecideApprove as &dyn SemOsVerbOp,
            "kyc_ubo.decide.subject.approve",
            serde_json::json!({}),
            false,
        ),
        (
            SubjectId(Uuid::new_v4()),
            &DecideReject as &dyn SemOsVerbOp,
            "kyc_ubo.decide.subject.reject",
            serde_json::json!({ "reason": "sanctions match confirmed" }),
            true,
        ),
    ] {
        if register_first {
            run(
                &KycSubjectPlace,
                serde_json::json!({ "subject-id": subject.0, "entity-type": "natural_person" }),
                &pool,
            )
            .await;
        }
        let mut args = serde_json::json!({ "subject-id": subject.0 });
        for (k, v) in extra_args.as_object().unwrap() {
            args[k] = v.clone();
        }
        run(op, args, &pool).await;

        let basis: serde_json::Value = sqlx::query_scalar(&format!(
            r#"SELECT basis FROM "ob-poc".kyc_decision_records
               WHERE subject_root = $1 AND verb_fqn = '{fqn}'"#
        ))
        .bind(subject.0)
        .fetch_one(&pool)
        .await
        .unwrap();
        let run_id: Uuid = basis
            .get("run_id")
            .and_then(|v| v.as_str())
            .and_then(|s| Uuid::parse_str(s).ok())
            .unwrap_or_else(|| panic!("{fqn}'s basis must cite a run_id: {basis:?}"));

        let (recorded_hash, recorded_subject): (String, Uuid) = sqlx::query_as(
            r#"SELECT board_state_hash, subject_root FROM "ob-poc".kyc_evaluation_runs
               WHERE run_id = $1"#,
        )
        .bind(run_id)
        .fetch_one(&pool)
        .await
        .unwrap_or_else(|e| panic!("{fqn} cited run {run_id} but no such row exists in kyc_evaluation_runs: {e}"));
        assert_eq!(
            recorded_subject, subject.0,
            "{fqn}'s cited run must belong to the subject it decided about"
        );
        assert_eq!(
            basis.get("board_state_hash").and_then(|v| v.as_str()),
            Some(recorded_hash.as_str()),
            "{fqn}'s basis.board_state_hash must match the cited run's own recorded hash"
        );

        cleanup(&pool, &[subject]).await;
    }
}
