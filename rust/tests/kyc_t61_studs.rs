//! T6.1 gate tests — EOP-PLAN-KYCUBO-KIT-001 §T6.1 (closes the ratified
//! EOP-DD-KYCUBO-KIT-T6 matrix rows 6a/8a) + the unified two-fold checker
//! (T6.1(a)).
//!
//! Four gates:
//! - `precondition_blocks_illegal_placement` / `precondition_admits_legal_placement`
//!   — the T6.1(c) exemplar (`StructureClassSupported`), at the checker
//!   level (pure, no DB — the same oracle the write path and the placement
//!   generator both call). TS.6 P2 (K-G7) retired `select-strategy` AND
//!   `compute-fold`, so both gates now drive `freeze` alone, the guard's
//!   sole surviving home.
//! - `checker_sees_both_folds` — a direct unit-level proof that
//!   `check_preconditions` genuinely evaluates an `ObligationState`-reading
//!   variant (`SubjectNotDecided`), with NO lexicon-entry attachment
//!   (unattached machinery, as T6.1(b) ships it).
//! - `kit_drift_on_stud_batch` — an open workbook pinned to a pre-T6.1 kit
//!   hash must KitDrift at commit against the live (post-T6.1) kit — the
//!   batch-then-reopen discipline (EOP-PLAN v0.6 fact 3), demonstrated as
//!   CORRECT behaviour, not a bug.
//!
//! `placement_iff_precondition_*` (the T2 property test) is unmodified and
//! re-runs green in `ob-poc-kyc-substrate/tests/placement.rs` — not
//! duplicated here.
//!
//! `select_strategy_blocked_end_to_end` RETIRED (TS.6 P2, K-G7): it proved
//! the 6a exemplar through the real governed append path for
//! `ubo.determination.select-strategy`, which no longer exists — the
//! strategy is now derived from `structure_class`, never separately
//! asserted. It was one of two known pre-existing carried reds through
//! TS.5 (a stale structure-class fixture message); the retirement closes
//! it rather than fixing the message, since there is no verb left to test.

use std::sync::Arc;

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::TransactionScope;
use ob_poc::domain_ops::kyc_workbook::KycWorkbook;
use ob_poc_kyc_substrate::{
    check_preconditions, assembly_lexicon, ControlState, EntityId, EntityType, EntityTypeRecord,
    EventId, FoldRegistry, ObligationState, StructureClass, SubjectId, TypeProofStatus,
    TypeRegistryState, V1FoldImpl,
};
use ob_poc_types::TransactionScopeId;

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
    r.register(assembly_lexicon().hash, Arc::new(V1FoldImpl));
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
// Pure, no DB — direct proof against the governed lexicon entry (`freeze`)
// that `StructureClassSupported` fail-closes on an unimplemented class and
// admits an implemented one.
//
// TS.6 P2 (K-G7): `select-strategy` AND `compute-fold` retired —
// `StructureClassSupported` now lives solely on `freeze`. EOP-VS-UBO-GAME-001
// T3 (§3.3, 2026-08-27) removed the `ReconciledProjection` half of that
// pair entirely — no reconciliation field remains to set.
fn control_with_class_strategized(class: StructureClass) -> ControlState {
    ControlState {
        registered: true,
        structure_class: Some(class),
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
    let lexicon = assembly_lexicon();
    let subject = SubjectId(Uuid::new_v4());
    let empty_obligation = ObligationState::default();

    // TS.4 fixture rework: the exemplar class walked TS.1 Trust → TS.2
    // Foundation → TS.3 StateOwned → Nominee; at TS.4 Nominee joined the
    // implemented set (NomineePierceStrategy) and NO strategy-less class
    // remains — the guard set is TOTAL. The guard is retained, not retired:
    // the illegal placement it now fail-closes is the UNCLASSIFIED subject —
    // which is exactly what an unknown/garbage wire string folds to
    // (`structure_class_from_payload` → None). TS.6 P2 retired both
    // `select-strategy` and `compute-fold`; `freeze` alone must still block
    // that state.
    let unclassified_ready = ControlState {
        registered: true,
        structure_class: None,
        ..Default::default()
    };
    let freeze_entry = lexicon.get("kyc_ubo.decide.determination.freeze").unwrap();
    let freeze_result = check_preconditions(
        freeze_entry,
        &unclassified_ready,
        &empty_obligation,
        &TypeRegistryState::default(),
        &probe(subject, "kyc_ubo.decide.determination.freeze"),
    );
    assert!(
        freeze_result.is_err(),
        "freeze must be blocked for an unclassified subject even with reconcile satisfied \
         (post-TS.4 fail-closed floor)"
    );
}

/// EOP-DD-UBO-DISPATCH-001 T4-close (2026-08-28): `freeze`'s admit-side
/// precondition is `EntityTypeSupportsStrategy` now, reading
/// `TypeRegistryState` (`EntityId(subject_root.0)`), not
/// `StructureClassSupported` reading `ControlState.structure_class` — an
/// empty `TypeRegistryState::default()` fails closed regardless of what
/// `structure_class` says, since nothing reads that field for dispatch any
/// longer (R5-historical only). This fixture was passing
/// `TypeRegistryState::default()` and only setting `structure_class`,
/// which silently broke this test at T4 (empty always blocks) even though
/// it compiles and the sibling `precondition_blocks_illegal_placement`
/// test still passes coincidentally (empty-blocks-empty on both the old
/// and new mechanism). Fixed the fixture, not the precondition: register a
/// real, live-dispatchable `EntityType` for the probed subject.
fn type_registry_with(subject: SubjectId, entity_type: EntityType) -> TypeRegistryState {
    let mut registry = TypeRegistryState::default();
    registry.types.insert(
        EntityId(subject.0),
        EntityTypeRecord {
            entity_type,
            proof: TypeProofStatus::Alleged,
            originating_event_id: EventId(Uuid::new_v4()),
            proof_event_id: None,
        },
    );
    registry
}

#[test]
fn precondition_admits_legal_placement() {
    let lexicon = assembly_lexicon();
    let subject = SubjectId(Uuid::new_v4());
    let empty_obligation = ObligationState::default();

    // PrivateCompany is in the pinned implemented-strategy set.
    let pc_state = control_with_class_strategized(StructureClass::PrivateCompany);
    let type_registry = type_registry_with(subject, EntityType::PrivateLimitedCompany);
    let freeze_entry = lexicon.get("kyc_ubo.decide.determination.freeze").unwrap();
    assert!(
        check_preconditions(
            freeze_entry,
            &pc_state,
            &empty_obligation,
            &type_registry,
            &probe(subject, "kyc_ubo.decide.determination.freeze"),
        )
        .is_ok(),
        "freeze must be admitted for a PrivateLimitedCompany-typed subject with reconcile \
         satisfied"
    );
}

// ── checker_sees_both_folds ─────────────────────────────────────────────────
//
// Direct proof that `check_preconditions` genuinely evaluates an
// ObligationState-reading variant — this test builds a synthetic entry
// (clone a real one, override `.preconditions`) rather than reaching for a
// wired-up verb, exactly per the plan's own instruction for this gate.
//
// Originally exercised `SubjectNotDecided` (then UNATTACHED to any lexicon
// entry in T6.1). `SubjectNotDecided` and `SubjectOverallState::Approved`/
// `Rejected` were retired TS.6 P2 (K-G7): `kyc_ubo.decide.subject.approve`/`kyc_ubo.decide.subject.reject`
// moved off the fact stream entirely (structural Evaluation-pack split,
// `ob-poc-kyc-decide`), so the substrate's pure fold can no longer see a
// decision to be "not yet decided" about — the finality check moved with
// them onto `kyc_decision_records`. Rewired onto `SubjectAllTerminal`
// instead (wired, live, and still ObligationState-reading) — same proof,
// a real precondition rather than a synthetic one.

#[test]
fn checker_sees_both_folds() {
    let lexicon = assembly_lexicon();
    let subject = SubjectId(Uuid::new_v4());
    let empty_control = ControlState::default();

    let mut synthetic_entry = lexicon.get("kyc_ubo.assert.entity.identity").unwrap().clone();
    synthetic_entry.preconditions = vec![ob_poc_kyc_substrate::Precondition::SubjectAllTerminal];

    // InProgress (default/no rollup) — not all-terminal — must reject.
    let not_terminal = ObligationState::default();
    let result = check_preconditions(
        &synthetic_entry,
        &empty_control,
        &not_terminal,
        &TypeRegistryState::default(),
        &probe(subject, "kyc_ubo.assert.entity.identity"),
    );
    assert!(
        result.is_err(),
        "SubjectAllTerminal must reject a subject with no rollup (defaults to InProgress)"
    );

    // AllTerminal — build a rollup AND a matching all-terminal obligation by
    // hand (obligation is otherwise unreachable from outside the crate
    // without a real event stream; `ObligationState`'s fields are all
    // `pub`) — must admit. `derive_subject_state` derives fresh from
    // `rollup.obligations`/`self.obligations` every call (it does not
    // consult the stored `overall_state` field at all any more — that
    // shortcut existed only for the now-retired Approved/Rejected
    // variants), so a rollup with an empty `obligations` list is
    // indistinguishable from InProgress regardless of what `overall_state`
    // is set to; a real terminal obligation must be present.
    let obligation_id = ob_poc_kyc_substrate::ObligationId(Uuid::new_v4());
    let by_event = ob_poc_kyc_substrate::EventId::new();
    let mut terminal = ObligationState::default();
    terminal.obligations.insert(
        obligation_id,
        ob_poc_kyc_substrate::ObligationTracks {
            obligation_id,
            basis: ob_poc_kyc_substrate::ObligationBasis {
                role: "director".into(),
                jurisdiction: None,
                cbu_role: None,
                source_event_id: by_event,
            },
            identity: ob_poc_kyc_substrate::TrackState::Satisfied { by_event },
            screening: ob_poc_kyc_substrate::TrackState::Satisfied { by_event },
            risk: ob_poc_kyc_substrate::TrackState::Satisfied { by_event },
            originating_event_id: by_event,
        },
    );
    terminal.subjects.insert(
        subject,
        ob_poc_kyc_substrate::SubjectRollup {
            subject_id: subject,
            obligations: vec![obligation_id],
        },
    );
    assert!(
        check_preconditions(
            &synthetic_entry,
            &empty_control,
            &terminal,
            &TypeRegistryState::default(),
            &probe(subject, "kyc_ubo.assert.entity.identity"),
        )
        .is_ok(),
        "SubjectAllTerminal must admit once ObligationState shows the subject AllTerminal — \
         proves the checker reads the obligation fold, not just control"
    );
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
        kit: assembly_lexicon(),
        // A hash that is deliberately NOT today's live assembly_lexicon().hash —
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
