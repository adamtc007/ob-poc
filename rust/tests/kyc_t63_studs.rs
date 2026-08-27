//! T6.3 gate tests — EOP-PLAN-KYCUBO-KIT-001 §T6.3 (determination family,
//! closes the ratified EOP-DD-KYCUBO-KIT-T6 matrix rows 6 (remainder), 7,
//! 10; row 8 is the unchanged 8a freeze guard, already gated in
//! `kyc_t61_studs.rs`. Row 9 was HALTED during execution — see below.).
//!
//! Eight gates (block/admit per stud), each driven through the REAL governed
//! append path (live DB) — the real `SemOsVerbOp::execute()` for the verb
//! under test, mirroring `kyc_t62_studs.rs`'s discipline.
//!
//! Row coverage:
//! - row 6 (`select-strategy`): RETIRED TS.6 P2 — the strategy is DERIVED
//!   from `structure_class`, never asserted, so no verb carries row 6's
//!   studs. See the row-6 note below.
//! - row 7 (`apply-smo-fallback`): RETIRED TS.6 §5 — SMO is PULLED on
//!   exhaustion by the traversal (TS.3 §4a), never asserted. Its
//!   [ReconciledProjection, StructureClassSupported] pair was never unique
//!   to it: `freeze` declares the identical pair (row 8a).
//! - row 9 (`kyc_ubo.assert.subject.register`) HALTED, not shipped: the ratified
//!   `NotAlreadyRegistered` (a bare `!state.registered` boolean) is
//!   incompatible with `register`'s real production usage — one call
//!   registers the subject entity itself, one MORE call per natural-person
//!   candidate shares the same subject_root, differentiated only by
//!   payload `entity_id` (`natural_persons_from_events`,
//!   `kyc_stream_ops.rs::UboDeterminationFreeze`). Attaching the stud as
//!   ratified would block every multi-person determination; left
//!   geometry-free pending a matrix amendment to a KEYED
//!   (subject_root, entity_id) check. `phase1_lexicon()`'s entry for this
//!   verb is unchanged from pre-T6.3.
//! - row 10 (`kyc_ubo.assert.subject.structure-class`): `SubjectRegistered` — block
//!   classify-structure on an unregistered subject.

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::{TransactionScope, VerbExecutionContext};
use ob_poc::domain_ops::kyc_stream_ops::{KycSubjectClassifyStructure, KycSubjectPlace};
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
        .expect("DB")
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

async fn register(scope: &mut Scope, subject: SubjectId) {
    KycSubjectPlace
        .execute(
            &serde_json::json!({ "subject-id": subject.0, "is_natural_person": false, "entity-type": "private_limited_company" }),
            &mut VerbExecutionContext::default(),
            scope,
        )
        .await
        .expect("register has no NotAlreadyRegistered violation on a fresh subject");
}

// `classify` helper removed with the row-7 tests (TS.6 §5): its only
// callers were the retired apply-smo-fallback gates.

// ── row 6 — RETIRED (TS.6 P2, K-G7) ─────────────────────────────────────────
//
// `ubo.determination.select-strategy` and its two studs' attachment to it
// (`SubjectRegistered`, `StructureClassified`) are gone — the strategy is
// now DERIVED from `structure_class` (`strategy_for_structure_class`),
// never separately asserted, so there is no verb left to carry row 6's
// preconditions. `StructureClassified`/bare `SubjectRegistered` remain
// evaluable T6.1(b) machinery (unattached, per that tranche's convention);
// `StructureClassSupported` (a strictly stronger check — it also fails
// closed on `None`) does the real work now, attached solely to `freeze`
// (`compute-fold` also retired TS.6 P2), exhaustively covered by
// `kyc_pack_closure.rs` and `kyc_t61_studs.rs`'s
// precondition_{blocks,admits}_* gates.

// ── row 7 — apply-smo-fallback — RETIRED (TS.6 §5, 2026-08-22) ─────────────
//
// `row7_apply_smo_fallback_{blocks,admits}_*` drove
// `ubo.determination.apply-smo-fallback` to prove its
// [ReconciledProjection, StructureClassSupported] pair was enforced at the
// real op. The verb is retired: SMO is PULLED on exhaustion by the traversal
// (`determination.rs` walks OfficerAppointment edges into the frontier and
// emits `Prong::SmoFallback` straight into `candidates`, TS.3 §4a), so
// asserting one was a second way to WRITE an answer the system computes.
//
// Nothing is lost from this matrix: the precondition pair was never unique to
// row 7. `kyc_ubo.decide.determination.freeze` declares the IDENTICAL pair and is
// exercised by row 8a here, by `kyc_t61_studs.rs`'s
// `precondition_{blocks,admits}_*` gates, and by `kyc_pack_closure.rs`'s
// totality sub-test. Deleting these two tests removes duplicate coverage,
// not coverage.

// ── row 9 — kyc_ubo.assert.subject.register: NotAlreadyRegistered ─────────────────────

#[tokio::test]
async fn row9_register_admits_fresh_subject() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let mut scope = Scope::begin(&pool).await;

    let result = KycSubjectPlace
        .execute(
            &serde_json::json!({ "subject-id": subject.0, "is_natural_person": false, "entity-type": "private_limited_company" }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_ok(),
        "register must be admitted for a fresh (never-registered) subject: {result:?}"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}

// row9_register_blocks_double_registration intentionally NOT written: row 9
// (`NotAlreadyRegistered`) was HALTED during T6.3 execution, not shipped —
// see the lexicon entry's own comment in `lexicon.rs`. A second
// `kyc_ubo.assert.subject.register` call on the same subject_root is a REAL production
// pattern (one call per natural-person candidate within a determination
// stream, see `kyc_m3_remediation.rs`'s
// `m3_1_freeze_differential_matches_ownership_prong_strategy` /
// `m4_control_prong_strategy_resolves_gp_statutory_control`), so it must
// stay legal; asserting it here would pin the wrong behaviour.

// ── row 10 — kyc_ubo.assert.subject.structure-class: SubjectRegistered ─────────────

#[tokio::test]
async fn row10_classify_structure_blocks_unregistered_subject() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let mut scope = Scope::begin(&pool).await;
    // Deliberately NOT registered.

    let result = KycSubjectClassifyStructure
        .execute(
            &serde_json::json!({ "subject-id": subject.0, "structure-class": "private_company" }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_err(),
        "classify-structure must be blocked for an unregistered subject: {result:?}"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}

#[tokio::test]
async fn row10_classify_structure_admits_registered_subject() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let mut scope = Scope::begin(&pool).await;
    register(&mut scope, subject).await;

    let result = KycSubjectClassifyStructure
        .execute(
            &serde_json::json!({ "subject-id": subject.0, "structure-class": "private_company" }),
            &mut VerbExecutionContext::default(),
            &mut scope,
        )
        .await;
    assert!(
        result.is_ok(),
        "classify-structure must be admitted for a registered subject: {result:?}"
    );
    scope.tx.rollback().await.unwrap();
    cleanup(&pool, subject).await;
}
