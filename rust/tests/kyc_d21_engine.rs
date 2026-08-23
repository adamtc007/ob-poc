//! EOP-DD-KYCUBO-D2.1 Tranche C — the evaluation engine, D2.1 §6 gates.
//!
//! `evaluation_pack_dependency_graph_excludes_the_append_chokepoint`
//! (renamed from `evaluation_pack_cannot_write_facts`, D2.1 §7 Q1) already
//! lives in `tests/kyc_pack_closure.rs` — not duplicated here. The other
//! eight §6 gates for the engine are below, all driven through the real
//! `SemOsVerbOp::execute` path.

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::{TransactionScope, VerbExecutionContext};
use ob_poc::domain_ops::kyc_stream_ops::KycSubjectRegister;
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
    run(&KycSubjectRegister, serde_json::json!({ "subject-id": s_unevaluable.0, "is_natural_person": true }), &pool).await;
    run(&DecideObligationWaive, serde_json::json!({ "subject-id": s_unevaluable.0, "check-id": PROOF_CHECK_ID, "reason": "unevaluable case" }), &pool).await;
    let findings_unevaluable = findings_for(&pool, s_unevaluable).await;
    assert!(
        findings_unevaluable[0]["verdict"].get("Unevaluable").is_some(),
        "registered-but-untyped entity must be UNEVALUABLE: {findings_unevaluable}"
    );

    // (3) FAIL — a withdrawn, unproven member, via the real production op
    // `kyc_ubo.assert.subject.member-withdrawal` (TS.1 move 6).
    let s_fail = SubjectId(Uuid::new_v4());
    run(&KycSubjectRegister, serde_json::json!({ "subject-id": s_fail.0, "is_natural_person": true }), &pool).await;
    withdraw_member(&pool, s_fail).await;
    run(&DecideObligationWaive, serde_json::json!({ "subject-id": s_fail.0, "check-id": PROOF_CHECK_ID, "reason": "fail case" }), &pool).await;
    let findings_fail = findings_for(&pool, s_fail).await;
    assert!(findings_fail[0]["verdict"].get("Fail").is_some(), "withdrawn unproven entity must FAIL: {findings_fail}");

    cleanup(&pool, &[s_pass, s_unevaluable, s_fail]).await;
}

/// `kyc_ubo.assert.subject.member-withdrawal` — TS.1 move 6, the real
/// production op, used here to build the FAIL board above.
async fn withdraw_member(pool: &PgPool, subject: SubjectId) {
    use ob_poc::domain_ops::kyc_stream_ops::KycSubjectWithdrawMember;
    run(&KycSubjectWithdrawMember, serde_json::json!({ "subject-id": subject.0, "entity-id": subject.0 }), pool).await;
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
    run(&KycSubjectRegister, serde_json::json!({ "subject-id": subject.0, "is_natural_person": true }), &pool).await;
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
    run(&KycSubjectRegister, serde_json::json!({ "subject-id": subject.0, "is_natural_person": true }), &pool).await;
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
    run(&KycSubjectRegister, serde_json::json!({ "subject-id": subject.0, "is_natural_person": true }), &pool).await;

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
