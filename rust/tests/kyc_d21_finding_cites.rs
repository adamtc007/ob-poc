//! D2.0 §4's own ratified language — "per check, a verdict plus **findings
//! citing the facts relied on**", and a fail verdict is "a finding, **citing
//! the facts it rests on**" — landed as `Finding.cites: Vec<EventId>`,
//! typed for exactly this, and hardcoded to `vec![]` at the sole call site
//! (`evaluation_checks`, `evaluation.rs:180`) for every check and every
//! verdict.
//!
//! Consequence, proven live 2026-08-24: a vacuous Pass (zero entities on
//! the board) and a real Pass (an entity registered, typed, and proven) are
//! byte-identical in the persisted `Finding` — same `check_id`, same empty
//! `cites`, same verdict. `in_scope_check_ids` cannot carry the distinction
//! (it records which CHECKS applied, not which ENTITIES were examined).
//!
//! These three gates drive the property through the real production
//! `DecideApprove`/`DecideObligationWaive` dispatch path, exactly as every
//! other D2.1-era gate does — no internal fixture construction.

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use dsl_runtime::{TransactionScope, VerbExecutionOutcome};
use ob_poc::domain_ops::kyc_stream_ops::{
    KycSubjectAssertType, KycSubjectRegister, KycSubjectWithdrawMember, UboEdgeAttachEvidence,
};
use ob_poc_kyc_decide::{test_verb_execution_context_with_session, DecideApprove, DecideObligationWaive};
use ob_poc_kyc_substrate::SubjectId;
use ob_poc_types::TransactionScopeId;
use sem_os_postgres::ops::SemOsVerbOp;

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
    fn scope_id(&self) -> TransactionScopeId { self.id }
    fn transaction(&mut self) -> &mut Transaction<'static, Postgres> { &mut self.tx }
    fn pool(&self) -> &PgPool { &self.pool }
}

async fn run_ok(op: &dyn SemOsVerbOp, args: serde_json::Value, pool: &PgPool) -> serde_json::Value {
    let mut ctx = test_verb_execution_context_with_session(Uuid::new_v4());
    let mut scope = Scope::begin(pool).await;
    let outcome = op.execute(&args, &mut ctx, &mut scope).await.unwrap_or_else(|e| panic!("{}: {e}", op.fqn()));
    scope.tx.commit().await.unwrap();
    match outcome {
        VerbExecutionOutcome::Record(v) => v,
        other => serde_json::json!({ "non_record_outcome": format!("{other:?}") }),
    }
}

async fn findings_for(pool: &PgPool, subject: SubjectId) -> serde_json::Value {
    sqlx::query_scalar(
        r#"SELECT findings FROM "ob-poc".kyc_evaluation_runs WHERE subject_root=$1 ORDER BY valid_time DESC LIMIT 1"#,
    )
    .bind(subject.0)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// The real `event_id` (not `seq`) of the most recent event on `subject`
/// with the given `verb_fqn` — what a citation must actually reference to
/// be resolvable in the fact stream.
async fn event_id_for(pool: &PgPool, subject: SubjectId, verb_fqn: &str) -> Uuid {
    let row = sqlx::query(
        r#"SELECT event_id FROM "ob-poc".kyc_intent_events WHERE subject_root=$1 AND verb_fqn=$2 ORDER BY seq DESC LIMIT 1"#,
    )
    .bind(subject.0)
    .bind(verb_fqn)
    .fetch_one(pool)
    .await
    .unwrap();
    row.get("event_id")
}

async fn cleanup(pool: &PgPool, subjects: &[SubjectId]) {
    for s in subjects {
        for t in ["kyc_decision_records", "kyc_evaluation_runs", "kyc_intent_events", "kyc_subject_streams", "kyc_control_edge_projection"] {
            let _ = sqlx::query(&format!(r#"DELETE FROM "ob-poc".{t} WHERE subject_root = $1"#)).bind(s.0).execute(pool).await;
        }
    }
}

/// Build a real, non-vacuous board: subject + one distinct entity,
/// registered, typed, and proven via the unscoped attach-evidence path
/// (D2.1 corrective tranche Item 3a). Returns the attach-evidence event's
/// real `event_id` — the fact this board's Pass must cite.
async fn build_real_proven_board(pool: &PgPool, subject: SubjectId, entity: Uuid) -> Uuid {
    run_ok(&KycSubjectRegister, serde_json::json!({ "subject-id": subject.0, "entity-id": entity, "is_natural_person": false }), pool).await;
    run_ok(&KycSubjectAssertType, serde_json::json!({ "subject-id": subject.0, "entity-id": entity, "entity-type": "private_limited_company" }), pool).await;
    run_ok(&UboEdgeAttachEvidence, serde_json::json!({ "subject-id": subject.0, "entity-id": entity }), pool).await;
    event_id_for(pool, subject, "kyc_ubo.assert.edge.evidence").await
}

/// D2.0 §4 property, RED today (`cites: vec![]` hardcoded at
/// `evaluation.rs:180`): a Pass over a board with a proven-type entity must
/// cite the real event that proved it.
#[tokio::test]
async fn a_real_pass_cites_its_evidence() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let entity = Uuid::new_v4();
    let evidence_event_id = build_real_proven_board(&pool, subject, entity).await;

    let mut ctx = test_verb_execution_context_with_session(Uuid::new_v4());
    let mut scope = Scope::begin(&pool).await;
    let result = DecideApprove.execute(&serde_json::json!({ "subject-id": subject.0 }), &mut ctx, &mut scope).await;
    assert!(result.is_ok(), "a fully-proven board must be approved: {result:?}");
    scope.tx.commit().await.unwrap();

    let findings = findings_for(&pool, subject).await;
    let arr = findings.as_array().expect("findings must be an array");
    assert_eq!(arr[0]["verdict"], serde_json::json!("Pass"), "sanity: must be a real Pass: {findings}");

    let cites: Vec<String> = serde_json::from_value(arr[0]["cites"].clone()).expect("cites must deserialize as a string array");
    assert!(
        !cites.is_empty(),
        "a Pass over a board with a proven-type entity must cite at least one event, got empty cites: {findings}"
    );
    assert!(
        cites.iter().any(|c| c == &evidence_event_id.to_string()),
        "cites must include the REAL attach-evidence event ({evidence_event_id}) that proved the entity's type, got: {cites:?}"
    );

    cleanup(&pool, &[subject]).await;
}

/// D2.0 §4 property, RED today: the zero-entity Pass and the real Pass must
/// differ in the persisted record — not merely in the `subject` field
/// value, which distinguishes nothing about the BOARD.
#[tokio::test]
async fn a_vacuous_pass_is_distinguishable() {
    let pool = pool().await;

    let real_subject = SubjectId(Uuid::new_v4());
    let entity = Uuid::new_v4();
    build_real_proven_board(&pool, real_subject, entity).await;
    let mut ctx = test_verb_execution_context_with_session(Uuid::new_v4());
    let mut scope = Scope::begin(&pool).await;
    DecideApprove.execute(&serde_json::json!({ "subject-id": real_subject.0 }), &mut ctx, &mut scope).await.expect("real board must approve");
    scope.tx.commit().await.unwrap();

    let vacuous_subject = SubjectId(Uuid::new_v4());
    let mut ctx2 = test_verb_execution_context_with_session(Uuid::new_v4());
    let mut scope2 = Scope::begin(&pool).await;
    DecideApprove.execute(&serde_json::json!({ "subject-id": vacuous_subject.0 }), &mut ctx2, &mut scope2).await.expect("vacuous board must approve (nothing to fail)");
    scope2.tx.commit().await.unwrap();

    let real_findings = findings_for(&pool, real_subject).await;
    let vacuous_findings = findings_for(&pool, vacuous_subject).await;

    let real_cites = real_findings.as_array().unwrap()[0]["cites"].as_array().cloned().unwrap_or_default();
    let vacuous_cites = vacuous_findings.as_array().unwrap()[0]["cites"].as_array().cloned().unwrap_or_default();

    assert!(vacuous_cites.is_empty(), "a genuinely vacuous board must cite nothing: {vacuous_findings}");
    assert!(
        !real_cites.is_empty(),
        "a real board's Pass must cite something, distinguishing it from the vacuous board's Pass. \
         real: {real_findings}\nvacuous: {vacuous_findings}"
    );
    assert_ne!(
        real_cites, vacuous_cites,
        "real and vacuous findings' cites must differ — this is the field that makes the two \
         records distinguishable without re-deriving the board"
    );

    cleanup(&pool, &[real_subject, vacuous_subject]).await;
}

/// D2.0 §4 property: every finding either carries citations or its verdict
/// explains the absence. `Unevaluable` citing nothing is coherent (its own
/// `reason` field already names what was missing or provisional) —
/// verified unchanged. `Pass` and `Fail` over a non-empty board rest on a
/// real fact and must cite it — RED today for both (hardcoded `vec![]`).
#[tokio::test]
async fn every_verdict_cites_something_or_says_why() {
    let pool = pool().await;

    // Pass: real proven entity.
    let pass_subject = SubjectId(Uuid::new_v4());
    let entity = Uuid::new_v4();
    build_real_proven_board(&pool, pass_subject, entity).await;
    run_ok(&DecideObligationWaive, serde_json::json!({ "subject-id": pass_subject.0, "check-id": PROOF_CHECK_ID, "reason": "cites property: Pass arm" }), &pool).await;
    let pass_findings = findings_for(&pool, pass_subject).await;
    let pass_arr = pass_findings.as_array().unwrap();
    assert_eq!(pass_arr[0]["verdict"], serde_json::json!("Pass"), "sanity: {pass_findings}");
    assert!(
        !pass_arr[0]["cites"].as_array().unwrap().is_empty(),
        "Pass over a non-empty board must cite something: {pass_findings}"
    );

    // Unevaluable: registered, no type asserted at all (FactAbsent).
    let unevaluable_subject = SubjectId(Uuid::new_v4());
    run_ok(&KycSubjectRegister, serde_json::json!({ "subject-id": unevaluable_subject.0, "is_natural_person": true }), &pool).await;
    run_ok(&DecideObligationWaive, serde_json::json!({ "subject-id": unevaluable_subject.0, "check-id": PROOF_CHECK_ID, "reason": "cites property: Unevaluable arm" }), &pool).await;
    let unevaluable_findings = findings_for(&pool, unevaluable_subject).await;
    let unevaluable_arr = unevaluable_findings.as_array().unwrap();
    assert!(unevaluable_arr[0]["verdict"].get("Unevaluable").is_some(), "sanity: {unevaluable_findings}");
    assert!(
        unevaluable_arr[0]["verdict"]["Unevaluable"].get("reason").is_some(),
        "Unevaluable must carry its own reason (already true, unrelated to cites): {unevaluable_findings}"
    );
    // Coherent to cite nothing — it examined nothing citable.

    // Fail: registered, type asserted (Alleged, never proven), then withdrawn.
    let fail_subject = SubjectId(Uuid::new_v4());
    run_ok(&KycSubjectRegister, serde_json::json!({ "subject-id": fail_subject.0, "is_natural_person": true }), &pool).await;
    run_ok(&KycSubjectAssertType, serde_json::json!({ "subject-id": fail_subject.0, "entity-id": fail_subject.0, "entity-type": "natural_person" }), &pool).await;
    let alleged_event_id = event_id_for(&pool, fail_subject, "kyc_ubo.assert.subject.type").await;
    run_ok(&KycSubjectWithdrawMember, serde_json::json!({ "subject-id": fail_subject.0, "entity-id": fail_subject.0 }), &pool).await;
    run_ok(&DecideObligationWaive, serde_json::json!({ "subject-id": fail_subject.0, "check-id": PROOF_CHECK_ID, "reason": "cites property: Fail arm" }), &pool).await;
    let fail_findings = findings_for(&pool, fail_subject).await;
    let fail_arr = fail_findings.as_array().unwrap();
    assert!(fail_arr[0]["verdict"].get("Fail").is_some(), "sanity: {fail_findings}");
    let fail_cites: Vec<String> = serde_json::from_value(fail_arr[0]["cites"].clone()).unwrap();
    assert!(
        !fail_cites.is_empty(),
        "Fail over an entity with a real (if unproven) type assertion on record must cite that \
         assertion — D2.0 §4: 'a fail verdict is a finding, citing the facts it rests on': {fail_findings}"
    );
    assert!(
        fail_cites.iter().any(|c| c == &alleged_event_id.to_string()),
        "Fail's citation must be the REAL alleged-type-assertion event ({alleged_event_id}), got: {fail_cites:?}"
    );

    cleanup(&pool, &[pass_subject, unevaluable_subject, fail_subject]).await;
}

/// P3: "a reader of the persisted run can tell 'nothing was in scope' from
/// 'everything checked out' without re-deriving the board." Falls out of
/// P2's design directly — `cites` empty on a `Pass` means the check
/// genuinely examined nothing; non-empty means it examined and confirmed
/// real facts. Proven for the two cases the property names:
///
/// - the genuinely EMPTY board (zero registered entities) — vacuous.
/// - the REAL board (one proven entity) — not vacuous.
///
/// A third case is checked and DOCUMENTED, not "fixed": a board where every
/// registered entity is withdrawn-but-proved also yields `cites: []`. This
/// is NOT the same ambiguity the property warns against — `ProvenTypeCheck`
/// skips withdrawn entities entirely (they cannot fail OR contribute to
/// Pass), so the check genuinely rested on nothing here too. The persisted
/// record is honest about what the check actually examined, which is the
/// property's real content — not "was the underlying board literally
/// empty," which `cites` was never claiming to answer.
#[tokio::test]
async fn p3_vacuous_case_is_self_evident_in_the_record() {
    let pool = pool().await;

    // Genuinely empty board.
    let empty_subject = SubjectId(Uuid::new_v4());
    let mut ctx1 = test_verb_execution_context_with_session(Uuid::new_v4());
    let mut scope1 = Scope::begin(&pool).await;
    DecideApprove.execute(&serde_json::json!({ "subject-id": empty_subject.0 }), &mut ctx1, &mut scope1).await.expect("empty board must approve");
    scope1.tx.commit().await.unwrap();
    let empty_findings = findings_for(&pool, empty_subject).await;
    assert!(
        empty_findings.as_array().unwrap()[0]["cites"].as_array().unwrap().is_empty(),
        "genuinely empty board: cites must be empty: {empty_findings}"
    );

    // Real board: one distinct, proven entity.
    let real_subject = SubjectId(Uuid::new_v4());
    let entity = Uuid::new_v4();
    build_real_proven_board(&pool, real_subject, entity).await;
    let mut ctx2 = test_verb_execution_context_with_session(Uuid::new_v4());
    let mut scope2 = Scope::begin(&pool).await;
    DecideApprove.execute(&serde_json::json!({ "subject-id": real_subject.0 }), &mut ctx2, &mut scope2).await.expect("real board must approve");
    scope2.tx.commit().await.unwrap();
    let real_findings = findings_for(&pool, real_subject).await;
    assert!(
        !real_findings.as_array().unwrap()[0]["cites"].as_array().unwrap().is_empty(),
        "real board with a proven entity: cites must be non-empty: {real_findings}"
    );

    // Documented edge case: every registered entity withdrawn-but-proved.
    // The check skips withdrawn entities unconditionally, so it examines
    // nothing here either — `cites: []` is consistent, not ambiguous.
    let withdrawn_proved_subject = SubjectId(Uuid::new_v4());
    let withdrawn_entity = Uuid::new_v4();
    run_ok(&KycSubjectRegister, serde_json::json!({ "subject-id": withdrawn_proved_subject.0, "entity-id": withdrawn_entity, "is_natural_person": true }), &pool).await;
    run_ok(&KycSubjectAssertType, serde_json::json!({ "subject-id": withdrawn_proved_subject.0, "entity-id": withdrawn_entity, "entity-type": "natural_person" }), &pool).await;
    run_ok(&UboEdgeAttachEvidence, serde_json::json!({ "subject-id": withdrawn_proved_subject.0, "entity-id": withdrawn_entity }), &pool).await;
    run_ok(&KycSubjectWithdrawMember, serde_json::json!({ "subject-id": withdrawn_proved_subject.0, "entity-id": withdrawn_entity }), &pool).await;
    let mut ctx3 = test_verb_execution_context_with_session(Uuid::new_v4());
    let mut scope3 = Scope::begin(&pool).await;
    DecideApprove.execute(&serde_json::json!({ "subject-id": withdrawn_proved_subject.0 }), &mut ctx3, &mut scope3).await.expect("withdrawn-but-proved board must approve");
    scope3.tx.commit().await.unwrap();
    let withdrawn_findings = findings_for(&pool, withdrawn_proved_subject).await;
    assert_eq!(
        withdrawn_findings.as_array().unwrap()[0]["verdict"], serde_json::json!("Pass"),
        "sanity: a withdrawn-but-proved-only board must still Pass (nothing left to examine): {withdrawn_findings}"
    );
    assert!(
        withdrawn_findings.as_array().unwrap()[0]["cites"].as_array().unwrap().is_empty(),
        "documented, intentional: a board where every entity is withdrawn (regardless of prior \
         proof) cites nothing — the check examines zero entities, same as a truly empty board: \
         {withdrawn_findings}"
    );

    cleanup(&pool, &[empty_subject, real_subject, withdrawn_proved_subject]).await;
}
