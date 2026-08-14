//! T6.1 gate tests — EOP-PLAN-KYCUBO-KIT-001 §T6.1 (closes the ratified
//! EOP-DD-KYCUBO-KIT-T6 matrix rows 6a/8a) + the unified two-fold checker
//! (T6.1(a)).
//!
//! Five gates:
//! - `precondition_blocks_illegal_placement` / `precondition_admits_legal_placement`
//!   — the T6.1(c) exemplar (`StructureClassSupported`) on `select-strategy`
//!   and `freeze`, at the checker level (pure, no DB — the same oracle the
//!   write path and the placement generator both call).
//! - `checker_sees_both_folds` — a direct unit-level proof that
//!   `check_preconditions` genuinely evaluates an `ObligationState`-reading
//!   variant (`SubjectNotDecided`), with NO lexicon-entry attachment
//!   (unattached machinery, as T6.1(b) ships it).
//! - `select_strategy_blocked_end_to_end` — the exemplar proven through the
//!   REAL governed append path (live DB), not just the pure checker.
//! - `kit_drift_on_stud_batch` — an open workbook pinned to a pre-T6.1 kit
//!   hash must KitDrift at commit against the live (post-T6.1) kit — the
//!   batch-then-reopen discipline (EOP-PLAN v0.6 fact 3), demonstrated as
//!   CORRECT behaviour, not a bug.
//!
//! `placement_iff_precondition_*` (the T2 property test) is unmodified and
//! re-runs green in `ob-poc-kyc-substrate/tests/placement.rs` — not
//! duplicated here.

use std::sync::Arc;

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::{TransactionScope, VerbExecutionContext};
use ob_poc::domain_ops::kyc_stream_ops::{
    KycSubjectClassifyStructure, KycSubjectRegister, UboDeterminationSelectStrategy,
};
use ob_poc::domain_ops::kyc_workbook::KycWorkbook;
use ob_poc_kyc_substrate::{
    check_preconditions, phase1_lexicon, ControlState, FoldRegistry, ObligationState,
    StructureClass, SubjectId, V1FoldImpl,
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
        .expect("DB")
}

fn v1_registry() -> FoldRegistry {
    let mut r = FoldRegistry::new();
    r.register(phase1_lexicon().hash, Arc::new(V1FoldImpl));
    r
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

async fn cleanup(pool: &PgPool, subject: SubjectId) {
    for t in ["kyc_intent_events", "kyc_subject_streams"] {
        let _ = sqlx::query(&format!(r#"DELETE FROM "ob-poc".{t} WHERE subject_root = $1"#))
            .bind(subject.0)
            .execute(pool)
            .await;
    }
    let _ = sqlx::query(r#"DELETE FROM "public".outbox WHERE idempotency_key LIKE $1"#)
        .bind(format!("{}:%", subject.0))
        .execute(pool)
        .await;
}

// ── precondition_blocks_illegal_placement / _admits_legal_placement ────────
//
// Pure, no DB — direct proof against the governed lexicon entries
// (`select-strategy`, `freeze`) that `StructureClassSupported` fail-closes on
// an unimplemented class and admits an implemented one.

// T6.3 row 6 (2026-08-12): select-strategy now ALSO carries SubjectRegistered
// + StructureClassified (attached alongside 6a's StructureClassSupported) —
// both helpers below must set `registered: true` too, or the fixtures below
// would incorrectly block on the ordering studs instead of proving the 6a/8a
// StructureClassSupported guard they target.
fn control_with_class(class: StructureClass) -> ControlState {
    ControlState {
        registered: true,
        structure_class: Some(class),
        ..Default::default()
    }
}

fn control_with_class_reconciled_and_strategized(class: StructureClass) -> ControlState {
    ControlState {
        registered: true,
        structure_class: Some(class),
        reconciliation_event_id: Some(ob_poc_kyc_substrate::EventId::new()),
        selected_strategy: Some("ownership_prong_strategy".to_string()),
        strategy_event_id: Some(ob_poc_kyc_substrate::EventId::new()),
        ..Default::default()
    }
}

fn probe(subject: SubjectId, verb_fqn: &str) -> ob_poc_kyc_substrate::IntentEvent {
    ob_poc_kyc_substrate::IntentEvent::new(
        subject,
        verb_fqn,
        ob_poc_kyc_substrate::Principal::test_analyst(),
        ob_poc_kyc_substrate::AuthorityRef("t61-test".into()),
        ob_poc_kyc_substrate::TargetBinding::for_subject(subject),
        serde_json::Value::Null,
        chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0).unwrap(),
    )
}

#[test]
fn precondition_blocks_illegal_placement() {
    let lexicon = phase1_lexicon();
    let subject = SubjectId(Uuid::new_v4());
    let empty_obligation = ObligationState::default();

    // Nominee is the LAST class with no implemented DeterminationStrategy
    // (EOP-PLAN-KYCUBO-KIT-001 v0.6 §TS.4 = K-8 piercing) — select-strategy
    // must fail-closed rather than silently proceed toward a wrong
    // determination. (TS.3 fixture fix: this exemplar originally used Trust
    // (TS.1), then Foundation (TS.2), then StateOwned, which joined the
    // implemented set via StateOwnedStrategy — Nominee stays unimplemented
    // until TS.4 (this exemplar moves again at TS.4), so the guard's block
    // semantics are unchanged, only the exemplar class moved.)
    let nominee_state = control_with_class(StructureClass::Nominee);
    let select_entry = lexicon.get("ubo.determination.select-strategy").unwrap();
    let result = check_preconditions(
        select_entry,
        &nominee_state,
        &empty_obligation,
        &probe(subject, "ubo.determination.select-strategy"),
    );
    assert!(
        result.is_err(),
        "select-strategy must be blocked for a Nominee-classified subject (6a exemplar)"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("no implemented determination"),
        "rejection must be attributable to the StructureClassSupported guard; got: {msg}"
    );

    // 8a: freeze carries the SAME guard as defense in depth, even with its
    // other two preconditions (ReconciledProjection/StrategySelected)
    // otherwise satisfied — the guard alone must still block Nominee.
    let nominee_ready = control_with_class_reconciled_and_strategized(StructureClass::Nominee);
    let freeze_entry = lexicon.get("ubo.determination.freeze").unwrap();
    let freeze_result = check_preconditions(
        freeze_entry,
        &nominee_ready,
        &empty_obligation,
        &probe(subject, "ubo.determination.freeze"),
    );
    assert!(
        freeze_result.is_err(),
        "freeze must be blocked for a Nominee-classified subject even with reconcile+strategy \
         satisfied (8a defense in depth)"
    );
}

#[test]
fn precondition_admits_legal_placement() {
    let lexicon = phase1_lexicon();
    let subject = SubjectId(Uuid::new_v4());
    let empty_obligation = ObligationState::default();

    // PrivateCompany is in the pinned implemented-strategy set.
    let pc_state = control_with_class(StructureClass::PrivateCompany);
    let select_entry = lexicon.get("ubo.determination.select-strategy").unwrap();
    assert!(
        check_preconditions(
            select_entry,
            &pc_state,
            &empty_obligation,
            &probe(subject, "ubo.determination.select-strategy"),
        )
        .is_ok(),
        "select-strategy must be admitted for a PrivateCompany-classified subject"
    );

    let pc_ready = control_with_class_reconciled_and_strategized(StructureClass::PrivateCompany);
    let freeze_entry = lexicon.get("ubo.determination.freeze").unwrap();
    assert!(
        check_preconditions(
            freeze_entry,
            &pc_ready,
            &empty_obligation,
            &probe(subject, "ubo.determination.freeze"),
        )
        .is_ok(),
        "freeze must be admitted for a PrivateCompany-classified subject with reconcile+strategy \
         satisfied"
    );
}

// ── checker_sees_both_folds ─────────────────────────────────────────────────
//
// Direct proof that `check_preconditions` genuinely evaluates an
// ObligationState-reading variant. `SubjectNotDecided` is UNATTACHED to any
// lexicon entry in T6.1 — this test builds a synthetic entry (clone a real
// one, override `.preconditions`) rather than reaching for a wired-up verb,
// exactly per the plan's own instruction for this gate.

#[test]
fn checker_sees_both_folds() {
    let lexicon = phase1_lexicon();
    let subject = SubjectId(Uuid::new_v4());
    let empty_control = ControlState::default();

    let mut synthetic_entry = lexicon.get("kyc.obligation.create").unwrap().clone();
    synthetic_entry.preconditions = vec![ob_poc_kyc_substrate::Precondition::SubjectNotDecided];

    // InProgress (default/no rollup) — not decided — must admit.
    let not_decided = ObligationState::default();
    assert!(
        check_preconditions(
            &synthetic_entry,
            &empty_control,
            &not_decided,
            &probe(subject, "kyc.obligation.create"),
        )
        .is_ok(),
        "SubjectNotDecided must admit a subject with no decision on record"
    );

    // Approved — decided — must reject. Build a rollup by hand (obligation
    // is otherwise unreachable from outside the crate without a real event
    // stream; `ObligationState`'s fields are all `pub`).
    let mut decided = ObligationState::default();
    decided.subjects.insert(
        subject,
        ob_poc_kyc_substrate::SubjectRollup {
            subject_id: subject,
            obligations: vec![],
            overall_state: ob_poc_kyc_substrate::SubjectOverallState::Approved {
                by_event: ob_poc_kyc_substrate::EventId::new(),
            },
            decision_event_id: Some(ob_poc_kyc_substrate::EventId::new()),
        },
    );
    let result = check_preconditions(
        &synthetic_entry,
        &empty_control,
        &decided,
        &probe(subject, "kyc.obligation.create"),
    );
    assert!(
        result.is_err(),
        "SubjectNotDecided must reject once ObligationState shows the subject Approved — \
         proves the checker reads the obligation fold, not just control"
    );
}

// ── select_strategy_blocked_end_to_end ──────────────────────────────────────
//
// The 6a exemplar proven through the REAL governed append path (live DB),
// not just the pure checker above.

#[tokio::test]
async fn select_strategy_blocked_end_to_end() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());

    // NOTE: a fresh `VerbExecutionContext` per call — `execution_id` seeds the
    // idempotency key (B3); reusing one `ctx` across calls would dedupe every
    // call after the first against the register event, short-circuiting
    // BEFORE fold+validate ever runs (the dedup check is step 2, ahead of
    // step 3's precondition check, in `PgKycEventStore::append`).
    let mut scope = Scope::begin(&pool).await;
    KycSubjectRegister
        .execute(
            &serde_json::json!({ "subject-id": subject.0, "is_natural_person": false }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await
        .expect("register has no preconditions");
    KycSubjectClassifyStructure
        .execute(
            // TS.3 fixture fix: StateOwned joined the implemented set — the
            // fail-closed end-to-end exemplar becomes nominee (until TS.4;
            // moves again at TS.4).
            &serde_json::json!({ "subject-id": subject.0, "structure-class": "nominee" }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await
        .expect("classify-structure has no preconditions");

    let result = UboDeterminationSelectStrategy
        .execute(
            &serde_json::json!({ "subject-id": subject.0, "strategy": "ownership_prong_strategy" }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_err(),
        "select-strategy on a Nominee-classified subject must be rejected through the real \
         governed append path (6a exemplar, end to end; TS.3 fixture fix — StateOwned is now \
         implemented, Nominee stays fail-closed until TS.4)"
    );
    scope.commit().await;

    cleanup(&pool, subject).await;
}

// ── kit_drift_on_stud_batch ──────────────────────────────────────────────────
//
// An open workbook pinned to a pre-T6.1 kit hash must KitDrift at commit
// against the live (post-T6.1) kit. This is the batch-then-reopen discipline
// (EOP-PLAN-KYCUBO-KIT-001 v0.6 fact 3) working as designed — demonstrated
// as CORRECT behaviour, not a regression. `KycWorkbook`'s fields are all
// `pub` within the crate boundary (T4 design), so a stale pin is
// constructible directly rather than needing a real time-travelled build.

#[tokio::test]
async fn kit_drift_on_stud_batch() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let registry = v1_registry();

    let stale_workbook = KycWorkbook {
        subject,
        committed: Vec::new(),
        staged: Vec::new(),
        kit: phase1_lexicon(),
        // A hash that is deliberately NOT today's live phase1_lexicon().hash —
        // standing in for "this session was opened before the T6.1 stud batch
        // landed".
        kit_hash: "0000000000000000000000000000000000000000000000000000000000000000".to_string(),
    };

    let mut scope = Scope::begin(&pool).await;
    let result = stale_workbook.commit(&mut scope, &registry).await;
    assert!(
        result.is_err(),
        "a workbook pinned to a stale kit hash must KitDrift at commit, never silently \
         substitute the live kit"
    );
    let msg = format!("{:?}", result.unwrap_err());
    assert!(
        msg.contains("KitDrift"),
        "rejection must be the named KitDrift variant, not some other failure; got: {msg}"
    );
    scope.tx.rollback().await.unwrap();

    cleanup(&pool, subject).await;
}
