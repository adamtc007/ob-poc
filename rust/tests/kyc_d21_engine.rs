//! EOP-DD-KYCUBO-D2.1 Tranche C — the evaluation engine, D2.1 §6 gates.
//!
//! **2026-08-24 corrective tranche (Item 6, this file's own instance of the
//! bug it now guards against):** the original header here said "the eight
//! D2.1 §6 gates" and treated that as the complete, correct count — an
//! undercount, silently absorbing the one D2.1 §6 gate never written
//! (`in_scope_set_reflects_the_board`, now a proper four-field deferral
//! below) without ever naming it. D2.1 §6 names NINE gates:
//! `a_check_produces_a_verdict`, `findings_reach_the_run_record`,
//! `in_scope_set_reflects_the_board`, `unevaluable_carries_its_reason`,
//! `k23_gate_can_fire`, `waived_check_was_in_scope` (six, "for the engine"),
//! plus `evaluation_pack_dependency_graph_excludes_the_append_chokepoint`,
//! `run_trigger_is_a_session_identity`, `test_mode_side_door_is_named`
//! (three, "for the boundary and the gates"). Of those nine:
//! `evaluation_pack_dependency_graph_excludes_the_append_chokepoint` lives
//! in `tests/kyc_pack_closure.rs` (not duplicated here); **eight** of the
//! remaining are below as real `#[tokio::test]`/`#[test]` functions; **one**
//! (`in_scope_set_reflects_the_board`) is a named, four-field deferral (see
//! below) rather than a test, because it genuinely cannot be exercised
//! through production without catalogue content that is out of scope.
//!
//! `unevaluable_is_not_fail_through_production` and
//! `work_list_is_derived_from_latest_run_through_production` are NOT among
//! D2.1's own nine §6 gates — they close two of D2.0's original nine §6
//! gates that `kyc_d21_gate_rehoming.rs` previously, falsely, claimed were
//! closed by this file's OTHER tests. See that file's header for what was
//! wrong and why.

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use dsl_runtime::{TransactionScope, VerbExecutionContext};
use ob_poc::domain_ops::kyc_stream_ops::KycSubjectPlace;
use ob_poc_kyc_decide::{test_verb_execution_context_with_session, DecideApprove, DecideObligationWaive};
use ob_poc_kyc_seam::append_in_scope;
use ob_poc_kyc_substrate::{
    assembly_lexicon, work_list_from_history, AuthorityRef, EvaluationRun, FoldRegistry, Finding,
    Hash, IntentEvent, Principal, RunTrigger, SubjectId, TargetBinding, V1FoldImpl, Verdict,
};
use ob_poc_types::TransactionScopeId;
use sem_os_postgres::ops::SemOsVerbOp;
use std::sync::Arc;

const PROOF_CHECK_ID: &str = "board.every-entity-has-a-proven-type";

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string())
}

async fn pool() -> PgPool {
    PgPoolOptions::new().max_connections(4).connect(&database_url()).await.expect("connect to test DB")
}

struct Scope {
    tx: Transaction<'static, Postgres>,
    pool: PgPool,
    id: TransactionScopeId,
}
impl Scope {
    async fn begin(p: &PgPool) -> Self {
        Self { tx: p.begin().await.unwrap(), pool: p.clone(), id: TransactionScopeId::new() }
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

/// Dispatch through a REAL session identity (D2.1 §7 Q3) — the production path.
async fn run(op: &dyn SemOsVerbOp, args: serde_json::Value, pool: &PgPool) {
    let mut ctx = test_verb_execution_context_with_session(Uuid::new_v4());
    let mut scope = Scope::begin(pool).await;
    op.execute(&args, &mut ctx, &mut scope).await.unwrap_or_else(|e| panic!("{}: {e}", op.fqn()));
    scope.tx.commit().await.unwrap();
}

fn historical_registry() -> FoldRegistry {
    let mut r = FoldRegistry::new();
    r.register(assembly_lexicon().hash, Arc::new(V1FoldImpl));
    r
}

/// Constructs a `place` event directly at the fold level, bypassing the
/// `SemOsVerbOp` layer where `entity-type` is a required arg
/// (`canonical_event_shape`'s `place` arm, via `required_string_arg`).
/// `fold_control`'s `place` arm sets `registered`/`registered_entity_ids`
/// from `entity_id` alone, independent of whether `entity_type` is present;
/// `fold_type_registry`'s `place` arm only inserts a type record when it
/// is. Omitting `entity_type` therefore reaches "registered but untyped" —
/// a state no REAL op-dispatched call can produce (the op layer enforces
/// the required arg) but the fold itself still represents correctly, since
/// nothing in the fold requires the two axes to land in the same event.
/// Used only by `unevaluable_is_not_fail_through_production` /
/// `unevaluable_flips_to_pass_through_production`, which test the
/// evaluation engine's `ProvenTypeCheck::FactAbsent`/`AllegedType`
/// distinction. Appends directly via the same `append_in_scope` chokepoint
/// every op uses, so this is still a real governed append, just built below
/// the op layer's arg validation rather than through it. (Previously used
/// the retired `register`/`type` FQNs to reach this state, framed as
/// "pre-T2 historical shape" — EOP-DD-UBO-CLEANOUT-001 T6 P4, 2026-09-07,
/// corrected this: after the clean start there is no historical stream
/// left to justify that framing, and `place` reaches the identical fold
/// state without naming a retired verb.)
async fn append_historical(verb_fqn: &str, subject: SubjectId, payload: serde_json::Value, pool: &PgPool) {
    let registry = historical_registry();
    let mut scope = Scope::begin(pool).await;
    let event = IntentEvent::new(
        subject,
        verb_fqn,
        Principal::test_analyst(),
        AuthorityRef("test.historical-shape".into()),
        TargetBinding::for_subject(subject),
        payload,
        chrono::Utc::now(),
    )
    .with_lexicon_hash(assembly_lexicon().hash);
    append_in_scope(&mut scope, &registry, &event, "", |_, _| Ok(())).await.unwrap();
    scope.tx.commit().await.unwrap();
}

async fn run_expect_err(op: &dyn SemOsVerbOp, args: serde_json::Value, pool: &PgPool) -> String {
    let mut ctx = test_verb_execution_context_with_session(Uuid::new_v4());
    let mut scope = Scope::begin(pool).await;
    let err = op.execute(&args, &mut ctx, &mut scope).await.expect_err(&format!("{} was expected to be refused", op.fqn()));
    let _ = scope.tx.rollback().await;
    err.to_string()
}

/// Dispatch with an EXPLICIT raw context (bypassing the test-fixtures
/// session builder) — used only by `run_trigger_is_a_session_identity` /
/// `test_mode_side_door_is_named` to prove what happens with no session.
async fn run_expect_err_raw_ctx(op: &dyn SemOsVerbOp, args: serde_json::Value, ctx: &mut VerbExecutionContext, pool: &PgPool) -> String {
    let mut scope = Scope::begin(pool).await;
    let err = op.execute(&args, ctx, &mut scope).await.expect_err(&format!("{} was expected to be refused", op.fqn()));
    let _ = scope.tx.rollback().await;
    err.to_string()
}

async fn cleanup(pool: &PgPool, subjects: &[SubjectId]) {
    for s in subjects {
        for t in ["kyc_intent_events", "kyc_subject_streams", "kyc_control_edge_projection", "kyc_decision_records", "kyc_evaluation_runs"] {
            let _ = sqlx::query(&format!(r#"DELETE FROM "ob-poc".{t} WHERE subject_root = $1"#)).bind(s.0).execute(pool).await;
        }
    }
}

async fn findings_for(pool: &PgPool, subject: SubjectId) -> serde_json::Value {
    sqlx::query_scalar(
        r#"SELECT findings FROM "ob-poc".kyc_evaluation_runs WHERE subject_root=$1 ORDER BY created_at DESC LIMIT 1"#,
    )
    .bind(subject.0)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// D2.1 §6 `a_check_produces_a_verdict` — the proof check, driven through
/// the production entry point, returns pass / fail / unevaluable against
/// three boards. Today unrepresentable before this tranche (no `evaluate()`
/// existed at all); now genuinely exercised via `DecideObligationWaive`
/// (the only op with no K-23 gate, so all three board shapes can run to
/// completion without being refused).
#[tokio::test]
async fn a_check_produces_a_verdict() {
    let pool = pool().await;

    // (1) PASS — empty board, nothing registered, vacuously true.
    let s_pass = SubjectId(Uuid::new_v4());
    run(&DecideObligationWaive, serde_json::json!({ "subject-id": s_pass.0, "check-id": PROOF_CHECK_ID, "reason": "pass case" }), &pool).await;
    let findings_pass = findings_for(&pool, s_pass).await;
    let verdict_pass = findings_pass[0]["verdict"].clone();
    assert_eq!(verdict_pass, serde_json::json!("Pass"), "empty board must PASS: {findings_pass}");

    // (2) UNEVALUABLE — an entity registered, no type ever asserted.
    let s_unevaluable = SubjectId(Uuid::new_v4());
    run(&KycSubjectPlace, serde_json::json!({ "subject-id": s_unevaluable.0, "entity-type": "natural_person" }), &pool).await;
    run(&DecideObligationWaive, serde_json::json!({ "subject-id": s_unevaluable.0, "check-id": PROOF_CHECK_ID, "reason": "unevaluable case" }), &pool).await;
    let findings_unevaluable = findings_for(&pool, s_unevaluable).await;
    assert!(
        findings_unevaluable[0]["verdict"].get("Unevaluable").is_some(),
        "registered-but-untyped entity must be UNEVALUABLE: {findings_unevaluable}"
    );

    // (3) FAIL — a withdrawn, unproven member, via the real production op
    // `kyc_ubo.assert.subject.member-withdrawal` (TS.1 move 6).
    let s_fail = SubjectId(Uuid::new_v4());
    run(&KycSubjectPlace, serde_json::json!({ "subject-id": s_fail.0, "entity-type": "natural_person" }), &pool).await;
    withdraw_member(&pool, s_fail).await;
    run(&DecideObligationWaive, serde_json::json!({ "subject-id": s_fail.0, "check-id": PROOF_CHECK_ID, "reason": "fail case" }), &pool).await;
    let findings_fail = findings_for(&pool, s_fail).await;
    assert!(findings_fail[0]["verdict"].get("Fail").is_some(), "withdrawn unproven entity must FAIL: {findings_fail}");

    cleanup(&pool, &[s_pass, s_unevaluable, s_fail]).await;
}

/// `kyc_ubo.assert.subject.member-withdrawal` — TS.1 move 6, the real
/// production op, used here to build the FAIL board above.
async fn withdraw_member(pool: &PgPool, subject: SubjectId) {
    use ob_poc::domain_ops::kyc_stream_ops::KycSubjectRemove;
    run(&KycSubjectRemove, serde_json::json!({ "subject-id": subject.0, "entity-id": subject.0 }), pool).await;
}

/// D2.1 §6 `findings_reach_the_run_record` — a run's persisted `findings`
/// column contains the verdict. Today (pre-Tranche-C) always `[]`; now
/// non-empty and structured.
#[tokio::test]
async fn findings_reach_the_run_record() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    run(&DecideObligationWaive, serde_json::json!({ "subject-id": subject.0, "check-id": PROOF_CHECK_ID, "reason": "findings reach the record" }), &pool).await;
    let findings = findings_for(&pool, subject).await;
    let arr = findings.as_array().expect("findings must be a JSON array");
    assert_eq!(arr.len(), 1, "the one in-scope check must produce exactly one finding: {findings}");
    assert_eq!(arr[0]["check_id"], serde_json::json!(PROOF_CHECK_ID));
    cleanup(&pool, &[subject]).await;
}

/// D2.1 §6 `unevaluable_carries_its_reason` — the persisted reason names
/// which distinction applied (alleged edge / alleged type / admission on an
/// alleged classification / fact absent). Today the type models all three;
/// nothing populated them until this tranche.
#[tokio::test]
async fn unevaluable_carries_its_reason() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    // Below-the-op-layer shape — see `append_historical`'s own doc: this
    // test needs FactAbsent (no type asserted at all), which a real
    // op-dispatched `place` call can no longer produce (the op layer
    // requires `entity-type`), but the fold itself still represents.
    append_historical(
        "kyc_ubo.assert.subject.place",
        subject,
        serde_json::json!({ "entity_id": subject.0 }),
        &pool,
    )
    .await;
    run(&DecideObligationWaive, serde_json::json!({ "subject-id": subject.0, "check-id": PROOF_CHECK_ID, "reason": "reason probe" }), &pool).await;
    let findings = findings_for(&pool, subject).await;
    let reason = findings[0]["verdict"]["Unevaluable"]["reason"].clone();
    assert!(
        reason.get("FactAbsent").is_some(),
        "an entity with no type asserted at all must carry a FactAbsent reason naming that distinction: {reason}"
    );
    assert!(
        reason["FactAbsent"]["what"].as_str().unwrap_or("").contains(&subject.0.to_string()),
        "the reason must name the specific offending entity: {reason}"
    );
    cleanup(&pool, &[subject]).await;
}

/// Reconstruct a subject's full `EvaluationRun` history from
/// `kyc_evaluation_runs`, oldest-to-newest — the shape
/// `work_list_from_history` requires. Built from OUTSIDE the crate under
/// test, using only its public types (`Hash::from_hex`, public
/// `EvaluationRun`/`RunTrigger` fields, `Finding`'s `Deserialize`), the same
/// "independent recomputation" discipline `kyc_d21_gate_rehoming.rs` uses —
/// this is the loader D2.0/D2.1 never needed because nothing before this
/// tranche read a run BACK out of the database as a typed history.
async fn load_run_history(pool: &PgPool, subject: SubjectId) -> Vec<EvaluationRun> {
    let rows = sqlx::query(
        r#"SELECT run_id, subject_root, board_state_hash, evaluation_pack_version_hash,
                  valid_time, knowledge_time, trigger, triggering_session,
                  in_scope_check_ids, findings
           FROM "ob-poc".kyc_evaluation_runs WHERE subject_root = $1 ORDER BY created_at ASC"#,
    )
    .bind(subject.0)
    .fetch_all(pool)
    .await
    .unwrap();
    rows.into_iter()
        .map(|row| EvaluationRun {
            run_id: row.get("run_id"),
            subject_root: SubjectId(row.get("subject_root")),
            board_state_hash: Hash::from_hex(&row.get::<String, _>("board_state_hash")).unwrap(),
            evaluation_pack_version_hash: Hash::from_hex(&row.get::<String, _>("evaluation_pack_version_hash")).unwrap(),
            valid_time: row.get("valid_time"),
            knowledge_time: row.get("knowledge_time"),
            trigger: RunTrigger { verb_fqn: row.get("trigger"), session_id: row.get("triggering_session") },
            in_scope_check_ids: serde_json::from_value(row.get("in_scope_check_ids")).unwrap(),
            findings: serde_json::from_value::<Vec<Finding>>(row.get("findings")).unwrap(),
        })
        .collect()
}

/// D2.1 §6 `unevaluable_is_not_fail`, rehomed through production.
///
/// The stated property (D2.0 §6, carried into D2.1): "a check whose facts
/// are absent returns unevaluable with a reason, never fail. Property:
/// adding the missing fact and re-running flips it to pass or fail, never
/// the reverse." `kyc_d21_gate_rehoming.rs`'s header previously claimed this
/// was closed by `a_check_produces_a_verdict`/`unevaluable_carries_its_reason`
/// — false: neither re-runs the SAME subject after a board change to
/// observe a flip. This test does.
///
/// This test exercises the Unevaluable -> Fail flip, via
/// `kyc_ubo.assert.subject.member-withdrawal` (a real, board-only fact that
/// makes a stuck proof permanently stuck). The Unevaluable -> Pass flip —
/// deferred at the 2026-08-23 reconciliation pending Item 3's Alleged ->
/// Proved wiring decision — is closed separately, in
/// `unevaluable_flips_to_pass_through_production` below, now that Item 3a
/// landed a dispatchable Proved path (2026-08-24 corrective tranche).
#[tokio::test]
async fn unevaluable_is_not_fail_through_production() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    // Below-the-op-layer shape — see `append_historical`'s own doc:
    // registered, no type yet.
    append_historical(
        "kyc_ubo.assert.subject.place",
        subject,
        serde_json::json!({ "entity_id": subject.0 }),
        &pool,
    )
    .await;

    // Run 1: no type asserted at all -> Unevaluable(FactAbsent). Never Fail.
    run(&DecideObligationWaive, serde_json::json!({ "subject-id": subject.0, "check-id": PROOF_CHECK_ID, "reason": "run 1: fact absent" }), &pool).await;
    let findings1 = findings_for(&pool, subject).await;
    assert!(findings1[0]["verdict"].get("Unevaluable").is_some(), "no type asserted must be Unevaluable, never Fail: {findings1}");
    assert!(findings1[0]["verdict"]["Unevaluable"]["reason"].get("FactAbsent").is_some());

    // Add a fact that does NOT resolve it (still Alleged) -> re-run -> STILL
    // Unevaluable, now for a DIFFERENT reason (AllegedType, not FactAbsent).
    // Confirms "never the reverse" isn't trivially satisfied by staying put
    // for the SAME reason.
    append_historical(
        "kyc_ubo.assert.subject.place",
        subject,
        serde_json::json!({ "entity_id": subject.0, "entity_type": "natural_person" }),
        &pool,
    )
    .await;
    run(&DecideObligationWaive, serde_json::json!({ "subject-id": subject.0, "check-id": PROOF_CHECK_ID, "reason": "run 2: still alleged" }), &pool).await;
    let findings2 = findings_for(&pool, subject).await;
    assert!(findings2[0]["verdict"].get("Unevaluable").is_some(), "an alleged type must stay Unevaluable: {findings2}");
    assert!(findings2[0]["verdict"]["Unevaluable"]["reason"].get("Provisional").is_some(), "must now be the AllegedType reason, not FactAbsent: {findings2}");

    // Add the fact that DOES resolve it (withdrawal — a real, board-only,
    // irreversible fact) -> re-run -> FLIPS to Fail. This is the flip the
    // property requires; it goes exactly one way.
    withdraw_member(&pool, subject).await;
    run(&DecideObligationWaive, serde_json::json!({ "subject-id": subject.0, "check-id": PROOF_CHECK_ID, "reason": "run 3: withdrawn, flips" }), &pool).await;
    let findings3 = findings_for(&pool, subject).await;
    assert!(findings3[0]["verdict"].get("Fail").is_some(), "withdrawal must flip Unevaluable -> Fail: {findings3}");

    cleanup(&pool, &[subject]).await;
}

/// Closes the deferral `unevaluable_is_not_fail_through_production` left
/// open: the Unevaluable -> Pass flip, now that Item 3a
/// (2026-08-24 corrective tranche) wired a dispatchable Proved path —
/// `kyc_ubo.assert.edge.evidence` called with `entity-id` (no `edge-id`)
/// evidences an entity's asserted type instead of an edge, logging a real
/// cited proof against it (EOP-DD-UBO-PROOF-001 §4, T5 — was "Alleged ->
/// Proved", now "uncited -> cited", `fold::type_registry`'s citation-set
/// arm). Re-runs the SAME subject through Unevaluable(FactAbsent) ->
/// Unevaluable(UncitedType) -> Pass, closing the property's other direction
/// (`unevaluable_is_not_fail_through_production` closed the -> Fail side).
#[tokio::test]
async fn unevaluable_flips_to_pass_through_production() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    // Below-the-op-layer shape — see `append_historical`'s own doc.
    append_historical(
        "kyc_ubo.assert.subject.place",
        subject,
        serde_json::json!({ "entity_id": subject.0 }),
        &pool,
    )
    .await;

    run(&DecideObligationWaive, serde_json::json!({ "subject-id": subject.0, "check-id": PROOF_CHECK_ID, "reason": "run 1: fact absent" }), &pool).await;
    let findings1 = findings_for(&pool, subject).await;
    assert!(findings1[0]["verdict"].get("Unevaluable").is_some(), "no type asserted must be Unevaluable: {findings1}");

    append_historical(
        "kyc_ubo.assert.subject.place",
        subject,
        serde_json::json!({ "entity_id": subject.0, "entity_type": "natural_person" }),
        &pool,
    )
    .await;
    run(&DecideObligationWaive, serde_json::json!({ "subject-id": subject.0, "check-id": PROOF_CHECK_ID, "reason": "run 2: alleged" }), &pool).await;
    let findings2 = findings_for(&pool, subject).await;
    assert!(findings2[0]["verdict"].get("Unevaluable").is_some(), "an alleged type must stay Unevaluable: {findings2}");

    // The fact that resolves it: real evidence attached to the ENTITY (not
    // an edge) — the dispatchable-Proved path Item 3a wired.
    use ob_poc::domain_ops::kyc_stream_ops::UboEdgeAttachEvidence;
    run(
        &UboEdgeAttachEvidence,
        serde_json::json!({
            "subject-id": subject.0, "entity-id": subject.0,
            "kind": "identity-document", "source": "test fixture", "date": "2026-08-28",
        }),
        &pool,
    )
    .await;
    run(&DecideObligationWaive, serde_json::json!({ "subject-id": subject.0, "check-id": PROOF_CHECK_ID, "reason": "run 3: proved, flips to pass" }), &pool).await;
    let findings3 = findings_for(&pool, subject).await;
    assert_eq!(findings3[0]["verdict"], serde_json::json!("Pass"), "a proven type must flip Unevaluable -> Pass: {findings3}");

    cleanup(&pool, &[subject]).await;
}

/// D2.1 §6 `work_list_is_derived_from_latest_run`, rehomed through
/// production.
///
/// The property (D2.0 §6): "structural: no stored work-list state exists;
/// the list is a query over the newest run." `kyc_d21_gate_rehoming.rs`'s
/// header previously claimed this closed too — false: `work_list_from_history`
/// was, before this test, called ONLY from the pure substrate fixture test
/// (`crates/ob-poc-kyc-substrate/tests/d20_evaluation.rs`), never against a
/// real persisted history. This builds TWO real runs on the same subject
/// whose VERDICTS DIFFER (Unevaluable, then Fail), reconstructs the history
/// from the database via `load_run_history`, and proves
/// `work_list_from_history` returns ONLY the latest run's finding — a bug
/// that concatenated both runs, or returned the first, would pass every
/// existing gate and fail only this one.
#[tokio::test]
async fn work_list_is_derived_from_latest_run_through_production() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    run(&KycSubjectPlace, serde_json::json!({ "subject-id": subject.0, "entity-type": "natural_person" }), &pool).await;

    // Run 1 (older): Unevaluable(FactAbsent).
    run(&DecideObligationWaive, serde_json::json!({ "subject-id": subject.0, "check-id": PROOF_CHECK_ID, "reason": "run 1" }), &pool).await;

    // Mutate the board so run 2's verdict genuinely differs: withdraw
    // (unproven) -> Fail.
    withdraw_member(&pool, subject).await;
    run(&DecideObligationWaive, serde_json::json!({ "subject-id": subject.0, "check-id": PROOF_CHECK_ID, "reason": "run 2" }), &pool).await;

    let history = load_run_history(&pool, subject).await;
    assert_eq!(history.len(), 2, "must have exactly two persisted runs");
    assert!(
        matches!(history[0].findings[0].verdict, Verdict::Unevaluable { .. }),
        "sanity: run 1 (oldest) must be Unevaluable"
    );
    assert!(
        matches!(history[1].findings[0].verdict, Verdict::Fail { .. }),
        "sanity: run 2 (latest) must be Fail — verdicts genuinely differ between runs"
    );

    let work_list = work_list_from_history(&history);
    assert_eq!(work_list.len(), 1, "work list must be exactly the latest run's findings, not both runs' combined: {work_list:?}");
    assert!(
        matches!(work_list[0].verdict, Verdict::Fail { .. }),
        "work list must reflect the LATEST run's verdict (Fail), not the older run's (Unevaluable): {work_list:?}"
    );

    cleanup(&pool, &[subject]).await;
}

/// D2.1 §6 `k23_gate_can_fire` — THE falsifiability test for the whole
/// tranche. A subject with a failing/unevaluable finding is REFUSED
/// approval. Prior to this tranche, the gate read an always-empty work
/// list and a subject with ZERO events was approved.
#[tokio::test]
async fn k23_gate_can_fire() {
    let pool = pool().await;

    // A registered-but-untyped entity produces Unevaluable -> non-empty
    // work list -> approve MUST be refused.
    let subject = SubjectId(Uuid::new_v4());
    run(&KycSubjectPlace, serde_json::json!({ "subject-id": subject.0, "entity-type": "natural_person" }), &pool).await;
    let err = run_expect_err(&DecideApprove, serde_json::json!({ "subject-id": subject.0 }), &pool).await;
    assert!(err.contains("K-23"), "approval of a subject with a non-empty work list must be refused citing K-23: {err}");

    // Contrast: an EMPTY board (the old defect's exact reproduction case)
    // still legitimately passes — the gate can fire AND not over-fire.
    let subject_empty = SubjectId(Uuid::new_v4());
    run(&DecideApprove, serde_json::json!({ "subject-id": subject_empty.0 }), &pool).await;

    cleanup(&pool, &[subject, subject_empty]).await;
}

/// D2.1 §6 `waived_check_was_in_scope` — a waiver names a check that was in
/// the cited run's in-scope set. Today `sanctions.screen` was waived; it
/// was never in scope, never evaluated, and does not exist.
#[tokio::test]
async fn waived_check_was_in_scope() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    run(&KycSubjectPlace, serde_json::json!({ "subject-id": subject.0, "entity-type": "natural_person" }), &pool).await;

    let err = run_expect_err(
        &DecideObligationWaive,
        serde_json::json!({ "subject-id": subject.0, "check-id": "not.a.real.check", "reason": "should be refused" }),
        &pool,
    )
    .await;
    assert!(err.contains("waived_check_was_in_scope"), "waiving a check never in scope must be refused: {err}");

    // Contrast: the REAL check_id succeeds.
    run(&DecideObligationWaive, serde_json::json!({ "subject-id": subject.0, "check-id": PROOF_CHECK_ID, "reason": "real check" }), &pool).await;

    cleanup(&pool, &[subject]).await;
}

/// D2.1 §6 `run_trigger_is_a_session_identity` — every run record carries
/// the session that triggered it (§7 Q3). A run with no session origin is
/// refused.
#[tokio::test]
async fn run_trigger_is_a_session_identity() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());

    // A bare default context has no session_id extension — must be refused.
    let mut bare_ctx = VerbExecutionContext::default();
    let err = run_expect_err_raw_ctx(
        &DecideObligationWaive,
        serde_json::json!({ "subject-id": subject.0, "check-id": PROOF_CHECK_ID, "reason": "no session" }),
        &mut bare_ctx,
        &pool,
    )
    .await;
    assert!(err.contains("session"), "a run with no session origin must be refused: {err}");

    // A context WITH a session_id succeeds and the persisted run carries it.
    let session_id = Uuid::new_v4();
    let mut ctx = test_verb_execution_context_with_session(session_id);
    let mut scope = Scope::begin(&pool).await;
    DecideObligationWaive
        .execute(&serde_json::json!({ "subject-id": subject.0, "check-id": PROOF_CHECK_ID, "reason": "with session" }), &mut ctx, &mut scope)
        .await
        .unwrap();
    scope.tx.commit().await.unwrap();

    let recorded_session: Uuid = sqlx::query_scalar(
        r#"SELECT triggering_session FROM "ob-poc".kyc_evaluation_runs WHERE subject_root=$1"#,
    )
    .bind(subject.0)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(recorded_session, session_id, "the persisted run must carry the SAME session identity the op ran under");

    cleanup(&pool, &[subject]).await;
}

/// D2.1 §6 `test_mode_side_door_is_named` — structural: the test-only run
/// path is explicit and unreachable from production. Proven by construction
/// (the Cargo feature graph), not by a runtime check — see
/// `crates/ob-poc-kyc-decide/Cargo.toml`'s `test-fixtures` feature and the
/// root workspace `Cargo.toml`'s `[dev-dependencies]` entry requesting it.
/// This test asserts BOTH halves of that construction hold.
#[test]
fn test_mode_side_door_is_named() {
    // (1) The side door has a name distinct from anything production calls
    // — `test_verb_execution_context_with_session` is imported at the top
    // of this very file, which only compiles because `cargo test` unifies
    // `[dev-dependencies]` features. If the workspace root's `[dev-dependencies]`
    // entry for ob-poc-kyc-decide (or the crate's own `test-fixtures`
    // feature) were removed, THIS FILE would fail to compile — that
    // compile-time dependency is the proof, not a runtime assertion.
    let _reachable_here_only_because_test_fixtures_is_active: fn(Uuid) -> VerbExecutionContext =
        test_verb_execution_context_with_session;

    // (2) It is grep-provable that the crate's own Cargo.toml gates the
    // function behind a feature not requested by [dependencies].
    let manifest = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/ob-poc-kyc-decide/Cargo.toml"))
        .expect("read ob-poc-kyc-decide/Cargo.toml");
    assert!(manifest.contains("test-fixtures = []"), "the test-fixtures feature must be declared");
    let lib_rs = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/crates/ob-poc-kyc-decide/src/lib.rs"))
        .expect("read ob-poc-kyc-decide/src/lib.rs");
    assert!(
        lib_rs.contains(r#"#[cfg(feature = "test-fixtures")]"#) && lib_rs.contains("pub fn test_verb_execution_context_with_session"),
        "the side door must be named AND cfg-gated, not a bare pub fn"
    );
}

// ══════════════════════════════════════════════════════════════════════════
// D2.1 §6 `in_scope_set_reflects_the_board` — NAMED DEFERRAL, not silently
// absent (2026-08-24 corrective tranche Item 2).
//
// WHAT: a gate driven through production proving the persisted
//       `in_scope_check_ids` responds to board content — e.g. adding a
//       trust to the board changes the set on the next run — as opposed to
//       merely agreeing with an independent recomputation of a STATIC
//       board (which is what `in_scope_set_is_computed_not_stored_through_production`
//       in `kyc_d21_gate_rehoming.rs` actually tests, and which catches a
//       fabricated/wrong set but cannot demonstrate RESPONSIVENESS to board
//       changes).
// WHY:  `EvaluationCatalogue::default_catalogue()` — the only catalogue any
//       production op ever consults — holds exactly one check
//       (`ProvenTypeCheck`), and its applicability is `Unconditional`
//       (always in scope, regardless of board content). There is no
//       conditional check in the real catalogue whose applicability could
//       ever change when a trust (or anything else) is added to the board,
//       so there is nothing for this property to observe end-to-end
//       through production. Authoring a conditional check to make the gate
//       observable is check-CATALOGUE-CONTENT — explicitly out of scope
//       (D2.0 §7 Q2, reaffirmed D2.1 §2's scope line: "what a particular
//       check concludes... is content and is out"). The MECHANISM this
//       property depends on (`ApplicabilityCondition::holds`,
//       `applicability_holds`'s OR-composition) is independently proven
//       correct and responsive to board mutation —
//       `entity_type_present_condition_reads_the_type_registry`
//       (`crates/ob-poc-kyc-substrate/tests/d20_evaluation.rs`) shows
//       `EntityTypePresent(Trust)` flips true/false exactly with the
//       board's folded type registry — but that is a pure, direct call, not
//       a run through a real op against a real catalogue, and asserting
//       otherwise would be exactly the "redefine the deliverable to match
//       what was built" failure this corrective tranche exists to close.
// WHO:  whoever authors the first real, non-`Unconditional` check in the
//       catalogue (same owner as D2.1 §4's existing "Check catalogue
//       contents" deferral — Adam + compliance).
// WHEN: falls out naturally the moment a second check with conditional
//       applicability lands in `EvaluationCatalogue::default_catalogue()` —
//       no separate ticket; write this gate in the same tranche that adds
//       that check.
// ══════════════════════════════════════════════════════════════════════════
