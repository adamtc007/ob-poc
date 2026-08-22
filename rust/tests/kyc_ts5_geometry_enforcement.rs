//! EOP-DD-KYCUBO-TS.5 §5 gate suite — the headline live-op gate, plus the
//! §4 tooth. Live-DB harness, same pattern as `kyc_ts4_pierce_traversal.rs`:
//! drives the REAL governed `kyc_ubo.assert.edge.control` op end-to-end, not a
//! direct call to `geometry::check_type_geometry`.
//!
//! Everything else in TS.5 §5 (target-side, preview/append agreement,
//! provisionality, error-shape distinguishability, correct-type regression)
//! is pure and lives in `crates/ob-poc-kyc-substrate/tests/ts5_geometry_enforcement.rs` —
//! `check_preconditions` needs no store, so there is no reason to pay for a
//! transaction on tests that don't need one.

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::{TransactionScope, VerbExecutionContext};
use ob_poc::domain_ops::kyc_stream_ops::{KycSubjectAssertType, KycSubjectRegister, UboEdgeAssertControl};
use ob_poc_types::TransactionScopeId;
use sem_os_postgres::ops::SemOsVerbOp;

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string())
}

async fn pool() -> PgPool {
    PgPoolOptions::new().max_connections(4).connect(&database_url()).await.expect("DB")
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
    async fn rollback(self) {
        self.tx.rollback().await.unwrap();
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

async fn register(scope: &mut Scope, subject: Uuid, entity: Uuid) {
    let mut ctx = VerbExecutionContext::default();
    KycSubjectRegister
        .execute(
            &serde_json::json!({ "subject-id": subject, "entity-id": entity }),
            &mut ctx,
            scope,
        )
        .await
        .unwrap_or_else(|e| panic!("register failed: {e}"));
}

async fn assert_type(scope: &mut Scope, subject: Uuid, entity: Uuid, wire: &str) {
    let mut ctx = VerbExecutionContext::default();
    KycSubjectAssertType
        .execute(
            &serde_json::json!({ "subject-id": subject, "entity-id": entity, "entity-type": wire }),
            &mut ctx,
            scope,
        )
        .await
        .unwrap_or_else(|e| panic!("assert-type failed: {e}"));
}

async fn assert_control(
    scope: &mut Scope,
    subject: Uuid,
    from: Uuid,
    to: Uuid,
    kind: &str,
) -> anyhow::Result<serde_json::Value> {
    let mut ctx = VerbExecutionContext::default();
    UboEdgeAssertControl
        .execute(
            &serde_json::json!({
                "subject-id": subject,
                "from_entity_id": from.to_string(),
                "to_entity_id": to.to_string(),
                "kind": kind,
            }),
            &mut ctx,
            scope,
        )
        .await
        .map(|v| match v {
            dsl_runtime::VerbExecutionOutcome::Record(r) => r,
            other => serde_json::to_value(format!("{other:?}")).unwrap(),
        })
}

/// The headline gate — the EXACT case proven admissible live 2026-08-21
/// (`ManagementMandate` sourced from a natural person): refused by the
/// governed op, live, not merely by a direct call to `check_type_geometry`.
/// Both endpoints must carry an asserted type (even alleged) for geometry
/// to be evaluable at all (R6) — the original investigation's probe never
/// asserted a type on either entity, so it was actually an (unrelated,
/// also-real) `Unevaluable`-shaped gap; this gate asserts types explicitly
/// so the refusal is a genuine geometry decision, not R6 admission.
#[tokio::test]
async fn illegal_source_type_is_refused_at_the_op() {
    let pool = pool().await;
    let subject = Uuid::new_v4();
    let natural_person = Uuid::new_v4();
    let fund_entity = Uuid::new_v4();

    let mut scope = Scope::begin(&pool).await;
    register(&mut scope, subject, natural_person).await;
    register(&mut scope, subject, fund_entity).await;
    assert_type(&mut scope, subject, natural_person, "natural_person").await;
    assert_type(&mut scope, subject, fund_entity, "oeic_icvc").await;

    let result = assert_control(&mut scope, subject, natural_person, fund_entity, "management_mandate").await;
    scope.rollback().await;

    let err = result.expect_err("expected the live governed op to REFUSE (TS.5 R1 landed)");
    let msg = err.to_string();
    eprintln!("illegal_source_type_is_refused_at_the_op: {msg}");
    assert!(
        msg.contains("geometry refused") || msg.contains("not a move"),
        "expected a geometry-shaped refusal, got: {msg}"
    );
}

/// §4's tooth: `every_geometry_rule_is_reachable_from_the_write_path` — one
/// illegal triple per §2a source restriction, driven through the REAL
/// `kyc_ubo.assert.edge.control` op, asserting refusal. RED before R1 landed
/// (proven in P0); green now; stays as the permanent guard against the
/// wiring being lost again.
#[tokio::test]
async fn every_geometry_rule_is_reachable_from_the_write_path() {
    let pool = pool().await;

    // (pipe wire, illegal source wire, legal target wire for that pipe) —
    // each row picks a source type §2a affirmatively excludes for that
    // pipe, and a target type §2 affirmatively permits it into, isolating
    // the SOURCE-side restriction under test.
    let cases: &[(&str, &str, &str, &str)] = &[
        ("gp_statutory", "GpDesignation excludes funds", "sicav", "general_partnership"),
        ("board_appointment", "BoardAppointment is natural-person-only", "private_limited_company", "private_limited_company"),
        ("officer_appointment", "OfficerAppointment is natural-person-only", "private_limited_company", "private_limited_company"),
        ("management_mandate", "ManagementMandate is corporate-only", "natural_person", "limited_partnership"),
        ("statutory_authority", "StatutoryAuthority is gov/sovereign-only", "private_limited_company", "government_dept_statutory_corporation"),
        ("employment", "EmploymentDelegatedAuthority is natural-person-only", "private_limited_company", "private_limited_company"),
        ("containment", "PooledAssetContainment is umbrella-only", "sicav", "umbrella_with_sub_funds"),
    ];

    for (kind, label, illegal_source_wire, legal_target_wire) in cases {
        let subject = Uuid::new_v4();
        let source = Uuid::new_v4();
        let target = Uuid::new_v4();
        let mut scope = Scope::begin(&pool).await;
        register(&mut scope, subject, source).await;
        register(&mut scope, subject, target).await;
        assert_type(&mut scope, subject, source, illegal_source_wire).await;
        assert_type(&mut scope, subject, target, legal_target_wire).await;

        let result = assert_control(&mut scope, subject, source, target, kind).await;
        scope.rollback().await;

        assert!(
            result.is_err(),
            "[{label}] kind={kind} source={illegal_source_wire} target={legal_target_wire}: \
             expected refusal, got {result:?}"
        );
        eprintln!("[{label}] refused as expected: {}", result.unwrap_err());
    }
}

// ── TS.5 §4: the wiring half of R5/R6 (added 2026-08-22) ────────────────────
//
// §4 ratifies that every rule component gets a wiring test through the
// PRODUCTION ENTRY POINT, not the function directly — that is the countermeasure
// for "built but not wired", the defect family member TS.5 itself named.
//
// The 2026-08-22 reconciliation found the R5/R6 gates in
// `crates/ob-poc-kyc-substrate/tests/ts5_geometry_enforcement.rs` call
// `check_preconditions` directly. That function IS the shared chokepoint (TS.5
// §1), so those gates are correct about the RULE — but they cannot see the op.
// A regression that stopped `UboEdgeAssertControl` from routing through
// `check_preconditions`, or that made the op fail closed on an unevaluable
// triple before ever reaching it, would leave that suite green while R5/R6 were
// broken in production. These two drive the real governed op instead.
//
// R5/R6 are ADMIT cases, so their wiring test asserts the op ADMITS. That is
// the direction CTN-2e cares about: enforcement narrows what may be ASSERTED,
// never what may be ALLEGED.

/// R6, wired: an endpoint with NO type asserted at all must still be admitted
/// by the real op — geometry is unevaluable, and unevaluable admits.
#[tokio::test]
async fn untyped_endpoint_admits_at_the_op() {
    let pool = pool().await;
    let subject = Uuid::new_v4();
    let a = Uuid::new_v4();
    let b = Uuid::new_v4();

    let mut scope = Scope::begin(&pool).await;
    register(&mut scope, subject, a).await;
    register(&mut scope, subject, b).await;
    // Deliberately NO assert_type for either endpoint.
    let verdict = assert_control(&mut scope, subject, a, b, "voting_rights").await;
    scope.rollback().await;

    assert!(
        verdict.is_ok(),
        "R6: an untyped endpoint must ADMIT at the governed op (geometry \
         unevaluable, CTN-2e) — a fail-closed regression here is invisible to \
         the pure gate, which calls check_preconditions directly: {verdict:?}"
    );
}

/// R5, wired: an ALLEGED (unproven) endpoint type must still be admitted by the
/// real op. Enforcement narrows what may be asserted, never what may be alleged.
#[tokio::test]
async fn alleged_type_admits_at_the_op() {
    let pool = pool().await;
    let subject = Uuid::new_v4();
    let person = Uuid::new_v4();
    let corp = Uuid::new_v4();

    let mut scope = Scope::begin(&pool).await;
    register(&mut scope, subject, person).await;
    register(&mut scope, subject, corp).await;
    // `assert_type` records an ALLEGED type — nothing here proves it.
    assert_type(&mut scope, subject, person, "natural_person").await;
    assert_type(&mut scope, subject, corp, "private_limited_company").await;
    let verdict = assert_control(&mut scope, subject, person, corp, "voting_rights").await;
    scope.rollback().await;

    assert!(
        verdict.is_ok(),
        "R5: a geometrically-legal triple on merely-ALLEGED types must ADMIT at \
         the governed op: {verdict:?}"
    );
}
