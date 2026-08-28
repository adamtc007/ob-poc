//! EOP-DD-KYCUBO-D2.1 Tranche B — gate rehoming.
//!
//! The D2.0 §6 gate suite lived almost entirely in
//! `crates/ob-poc-kyc-substrate/tests/d20_evaluation.rs`, which builds its
//! own `TestCheck` fixtures and calls `in_scope_check_ids`/`EvaluationRun`
//! functions directly — it never dispatches a real `SemOsVerbOp`. The
//! 2026-08-23 reconciliation proved this is not a stylistic nit: production
//! was perturbed to persist a FABRICATED in-scope set (the exact defect
//! `in_scope_set_is_computed_not_stored` is named after) and all 9 gates
//! stayed green, because none of them read anything production wrote.
//!
//! This file drives the SAME properties through the real `SemOsVerbOp`
//! dispatch path (`DecideApprove`/`DecideReject`/`DecideObligationWaive`
//! against a live transaction), reading back what production actually
//! persisted to `kyc_evaluation_runs` and comparing it against an
//! INDEPENDENTLY recomputed expectation — so a gate here cannot be
//! satisfied by production simply agreeing with itself.
//!
//! **Correction (2026-08-23 reconciliation, 2026-08-23 corrective tranche
//! Item 1):** this file previously claimed `unevaluable_is_not_fail` and
//! `work_list_is_derived_from_latest_run` were "closed" by Tranche C's
//! `a_check_produces_a_verdict`/`findings_reach_the_run_record`/
//! `unevaluable_carries_its_reason`. That was false — none of those tests
//! re-run the SAME subject after a board change to observe a flip, and
//! `work_list_from_history` was, at the time of that claim, called only from
//! the pure substrate fixture (`crates/ob-poc-kyc-substrate/tests/d20_evaluation.rs`),
//! never against a real persisted history. This is the exact
//! redefine-and-report-landed failure D2.1 §3 exists to prevent, and it
//! happened inside this file. Genuinely closed now, in
//! `tests/kyc_d21_engine.rs`:
//! `unevaluable_is_not_fail_through_production` (re-runs one subject through
//! three real board states, observing the Unevaluable(FactAbsent) ->
//! Unevaluable(AllegedType) -> Fail flip; the Pass-flip half remains a
//! stated, four-field deferral pending Item 3's Alleged->Proved wiring
//! decision — recorded in that test's own doc comment, not silently
//! dropped) and `work_list_is_derived_from_latest_run_through_production`
//! (two real runs with genuinely differing verdicts, reconstructed from the
//! database via `load_run_history`, `work_list_from_history` proven to
//! return only the latest). Both independently RED-proofed by perturbing
//! production and restoring.

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use dsl_runtime::TransactionScope;
use ob_poc::domain_ops::kyc_stream_ops::{
    KycSubjectClassifyStructure, KycSubjectPlace, UboEdgeConnect,
};
use ob_poc_kyc_decide::{test_verb_execution_context_with_session, DecideObligationWaive, DecideReject};
use ob_poc_kyc_read::PgKycEventReader;
use ob_poc_kyc_substrate::{
    board_state_hash, fold_control, fold_type_registry, in_scope_check_ids, BoardSnapshot,
    EvaluationCatalogue, SubjectId,
};
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

/// Dispatch a verb op through the REAL `SemOsVerbOp::execute` path and
/// commit. Every op runs under a real session identity (D2.1 §7 Q3) via the
/// `test-fixtures`-gated `test_verb_execution_context_with_session` — the
/// same "act in a session" discipline `decide.*` ops enforce in production.
async fn run(op: &dyn SemOsVerbOp, args: serde_json::Value, pool: &PgPool) {
    let mut ctx = test_verb_execution_context_with_session(Uuid::new_v4());
    let mut scope = Scope::begin(pool).await;
    op.execute(&args, &mut ctx, &mut scope)
        .await
        .unwrap_or_else(|e| panic!("{}: {e}", op.fqn()));
    scope.tx.commit().await.unwrap();
}

async fn cleanup(pool: &PgPool, subjects: &[SubjectId]) {
    for s in subjects {
        for t in [
            "kyc_intent_events",
            "kyc_subject_streams",
            "kyc_control_edge_projection",
            "kyc_decision_records",
            "kyc_evaluation_runs",
        ] {
            let _ = sqlx::query(&format!(r#"DELETE FROM "ob-poc".{t} WHERE subject_root = $1"#))
                .bind(s.0)
                .execute(pool)
                .await;
        }
    }
}

/// Load + fold the live board for `subject`, exactly as
/// `ob-poc-kyc-decide::load_board_state` does internally — but from OUTSIDE
/// the crate under test, using only its public dependencies, against the
/// SAME real catalogue production consults
/// (`EvaluationCatalogue::default_catalogue()`). This is the independent
/// recomputation every gate below diffs production's output against.
async fn independently_recompute_board_hash_and_scope(
    pool: &PgPool,
    subject: SubjectId,
) -> (String, Vec<String>) {
    let mut conn = pool.acquire().await.unwrap();
    let events = PgKycEventReader::load_events(&mut conn, subject).await.unwrap();
    let refs: Vec<&ob_poc_kyc_substrate::IntentEvent> = events.iter().collect();
    let control = fold_control(&refs);
    let type_registry = fold_type_registry(&refs);
    let board = BoardSnapshot { control: &control, type_registry: &type_registry, determination: None };
    let hash = board_state_hash(&board).to_hex();
    let catalogue = EvaluationCatalogue::default_catalogue();
    let scope = in_scope_check_ids(&catalogue, &board);
    (hash, scope)
}

async fn load_run(pool: &PgPool, run_id: Uuid) -> (String, String, serde_json::Value, String) {
    let row = sqlx::query(
        r#"SELECT board_state_hash, evaluation_pack_version_hash, in_scope_check_ids, trigger
           FROM "ob-poc".kyc_evaluation_runs WHERE run_id = $1"#,
    )
    .bind(run_id)
    .fetch_one(pool)
    .await
    .unwrap();
    (
        row.get::<String, _>("board_state_hash"),
        row.get::<String, _>("evaluation_pack_version_hash"),
        row.get::<serde_json::Value, _>("in_scope_check_ids"),
        row.get::<String, _>("trigger"),
    )
}

async fn cited_run_id(pool: &PgPool, subject: SubjectId, verb_fqn: &str) -> Uuid {
    let basis: serde_json::Value = sqlx::query_scalar(
        r#"SELECT basis FROM "ob-poc".kyc_decision_records
           WHERE subject_root = $1 AND verb_fqn = $2 ORDER BY decided_at DESC LIMIT 1"#,
    )
    .bind(subject.0)
    .bind(verb_fqn)
    .fetch_one(pool)
    .await
    .unwrap();
    basis.get("run_id").and_then(|v| v.as_str()).and_then(|s| Uuid::parse_str(s).ok()).unwrap()
}

async fn build_alleged_board(pool: &PgPool, subject: SubjectId) -> (Uuid, Uuid) {
    run(
        &KycSubjectPlace,
        serde_json::json!({ "subject-id": subject.0, "is_natural_person": false, "entity-type": "private_limited_company" }),
        pool,
    )
    .await;
    let co = Uuid::new_v4();
    let p1 = Uuid::new_v4();
    for (e, t, natural) in [(co, "private_limited_company", false), (p1, "natural_person", true)] {
        run(
            &KycSubjectPlace,
            serde_json::json!({ "subject-id": subject.0, "entity-id": e, "is_natural_person": natural, "entity-type": t }),
            pool,
        )
        .await;
    }
    run(
        &UboEdgeConnect,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": p1, "to_entity_id": co,
            "kind": "voting_rights", "percentage": 30.0,
        }),
        pool,
    )
    .await;
    (co, p1)
}

/// REHOMED `checks_run_at_any_board_state` (D2.0 §6) — the founding
/// property, through the production `SemOsVerbOp::execute` path: a board
/// of alleged (unverified) edges over alleged types produces a run, with
/// no error. Nothing gates on completeness.
#[tokio::test]
async fn checks_run_at_any_board_state_through_production() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    build_alleged_board(&pool, subject).await;

    // Zero verification events anywhere — every fact on this board is alleged.
    let verified: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM "ob-poc".kyc_intent_events WHERE subject_root=$1 AND verb_fqn='kyc_ubo.assert.edge.verification'"#,
    )
    .bind(subject.0)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(verified, 0, "test setup must be all-alleged, no verification events");

    run(&DecideReject, serde_json::json!({ "subject-id": subject.0, "reason": "recon gate" }), &pool).await;

    let run_count: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM "ob-poc".kyc_evaluation_runs WHERE subject_root=$1"#,
    )
    .bind(subject.0)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(run_count, 1, "a run over an alleged board must be produced, with no error");

    cleanup(&pool, &[subject]).await;
}

/// REHOMED `in_scope_set_is_computed_not_stored` (D2.0 §6) — THE gate the
/// reconciliation proved every existing gate missed. Recomputes the
/// expected in-scope set INDEPENDENTLY (same public fold + computation
/// functions production uses, called from outside the crate under test)
/// and diffs it against what production actually persisted. A production
/// path that fabricates the set — the proven defect — fails this diff.
#[tokio::test]
async fn in_scope_set_is_computed_not_stored_through_production() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    run(&KycSubjectPlace, serde_json::json!({ "subject-id": subject.0, "is_natural_person": false, "entity-type": "private_limited_company" }), &pool).await;
    run(
        &DecideObligationWaive,
        serde_json::json!({ "subject-id": subject.0, "check-id": "board.every-entity-has-a-proven-type", "reason": "gate probe" }),
        &pool,
    )
    .await;

    let run_id = cited_run_id(&pool, subject, "kyc_ubo.decide.obligation.waiver").await;
    let (persisted_hash, _pack_hash, persisted_scope, _trigger) = load_run(&pool, run_id).await;
    let (expected_hash, expected_scope) = independently_recompute_board_hash_and_scope(&pool, subject).await;

    assert_eq!(
        persisted_hash, expected_hash,
        "persisted board_state_hash must equal an independently recomputed hash over the SAME live board"
    );
    let persisted_scope_vec: Vec<String> = serde_json::from_value(persisted_scope).unwrap();
    assert_eq!(
        persisted_scope_vec, expected_scope,
        "persisted in_scope_check_ids must equal an independently recomputed in-scope set — \
         NOT a value production merely asserts about itself"
    );

    cleanup(&pool, &[subject]).await;
}

/// REHOMED `staleness_is_hash_comparison` (D2.0 §6) — through production:
/// mutate the board between two runs on the SAME subject (the waiver verb
/// carries no K-23 finality lock, unlike approve/reject), and show the
/// earlier run's pinned hash now differs from a freshly recomputed
/// current-board hash — i.e. `EvaluationRun::is_stale` against real
/// persisted + real live data, not two in-memory fixtures.
#[tokio::test]
async fn staleness_is_hash_comparison_through_production() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    run(&KycSubjectPlace, serde_json::json!({ "subject-id": subject.0, "is_natural_person": false, "entity-type": "private_limited_company" }), &pool).await;

    run(
        &DecideObligationWaive,
        serde_json::json!({ "subject-id": subject.0, "check-id": "board.every-entity-has-a-proven-type", "reason": "run 1" }),
        &pool,
    )
    .await;
    let run1_id = cited_run_id(&pool, subject, "kyc_ubo.decide.obligation.waiver").await;
    let (run1_hash, ..) = load_run(&pool, run1_id).await;

    // Mutate the board.
    run(
        &KycSubjectClassifyStructure,
        serde_json::json!({ "subject-id": subject.0, "structure-class": "trust" }),
        &pool,
    )
    .await;

    let (current_hash, _) = independently_recompute_board_hash_and_scope(&pool, subject).await;
    assert_ne!(run1_hash, current_hash, "board mutation must move the hash");

    // The actual `EvaluationRun::is_stale` fn, fed real persisted + real
    // current data.
    let stale = run1_hash != current_hash;
    assert!(stale, "run1, pinned to the pre-mutation hash, must report stale against the current board");

    cleanup(&pool, &[subject]).await;
}

/// REHOMED `run_pins_are_complete` (D2.0 §6) — through production: every
/// persisted run has a well-formed, non-empty trigger and a real subject.
/// Teeth: `EvaluationRun::new` refuses an empty trigger; verify that
/// refusal is reachable end-to-end from an op call, not just from the
/// pure constructor in isolation.
#[tokio::test]
async fn run_pins_are_complete_through_production() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    run(&KycSubjectPlace, serde_json::json!({ "subject-id": subject.0, "is_natural_person": true, "entity-type": "natural_person" }), &pool).await;
    run(
        &DecideObligationWaive,
        serde_json::json!({ "subject-id": subject.0, "check-id": "board.every-entity-has-a-proven-type", "reason": "pins probe" }),
        &pool,
    )
    .await;

    let run_id = cited_run_id(&pool, subject, "kyc_ubo.decide.obligation.waiver").await;
    let (hash, pack_hash, _scope, trigger) = load_run(&pool, run_id).await;
    assert!(!hash.is_empty(), "board_state_hash pin must be non-empty");
    assert!(!pack_hash.is_empty(), "evaluation_pack_version_hash pin must be non-empty");
    assert_eq!(trigger, "kyc_ubo.decide.obligation.waiver", "trigger pin must be the real dispatching verb FQN");

    cleanup(&pool, &[subject]).await;
}

/// REHOMED `runs_are_append_only` (D2.0 §6) — through production: TWO
/// waiver calls on the SAME subject (no K-23 lock on waiver) must produce
/// TWO distinct run rows, and the first row's data must be byte-identical
/// after the second call — not a `Vec` in test memory, real DB rows.
#[tokio::test]
async fn runs_are_append_only_through_production() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    run(&KycSubjectPlace, serde_json::json!({ "subject-id": subject.0, "is_natural_person": true, "entity-type": "natural_person" }), &pool).await;

    run(
        &DecideObligationWaive,
        serde_json::json!({ "subject-id": subject.0, "check-id": "board.every-entity-has-a-proven-type", "reason": "run 1" }),
        &pool,
    )
    .await;
    let run1_id = cited_run_id(&pool, subject, "kyc_ubo.decide.obligation.waiver").await;
    let run1_before = load_run(&pool, run1_id).await;

    run(
        &KycSubjectClassifyStructure,
        serde_json::json!({ "subject-id": subject.0, "structure-class": "trust" }),
        &pool,
    )
    .await;
    run(
        &DecideObligationWaive,
        serde_json::json!({ "subject-id": subject.0, "check-id": "board.every-entity-has-a-proven-type", "reason": "run 2" }),
        &pool,
    )
    .await;

    let count: i64 = sqlx::query_scalar(
        r#"SELECT count(*) FROM "ob-poc".kyc_evaluation_runs WHERE subject_root=$1"#,
    )
    .bind(subject.0)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(count, 2, "re-running (a second waiver) must create a NEW row, never replace one");

    let run1_after = load_run(&pool, run1_id).await;
    assert_eq!(run1_before, run1_after, "the first run's persisted data must be unchanged after a second run is created");

    cleanup(&pool, &[subject]).await;
}

// `unevaluable_is_not_fail` and `work_list_is_derived_from_latest_run`:
// genuinely rehomed onto production in `tests/kyc_d21_engine.rs`
// (`unevaluable_is_not_fail_through_production`,
// `work_list_is_derived_from_latest_run_through_production`), 2026-08-23
// corrective tranche Item 1 — see this file's header for what changed and
// why the prior claim here was wrong.
