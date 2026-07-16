//! EOP-DD-KYCUBO-003 Phase M3 — RED-first tests for the determination-logic and
//! approval-gate remediation.
//!
//! These tests target gaps R1 (freeze bypassed OwnershipProngStrategy), R2
//! (person.approve had no K-23 gate), and R3 (structure_class payload-key
//! mismatch) found during the 2026-07-01 review of EOP-VS-KYCUBO-001 v0.6.
//! Written against the *pre-fix* code they fail; against the fixed code
//! (this commit) they pass.

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::{TransactionScope, VerbExecutionContext};
use ob_poc::domain_ops::kyc_stream_ops::{
    KycObligationCreate, KycPersonApprove, KycSubjectClassifyStructure, KycSubjectRegister,
    UboDeterminationApplySmoFallback, UboDeterminationComputeFold, UboDeterminationFreeze,
    UboDeterminationSelectStrategy, UboEdgeAssertControl, UboEdgeAssertEconomicInterest,
    UboEdgeReconcileConflict,
};
use ob_poc_kyc_store::PgKycEventStore;
use ob_poc_kyc_substrate::{
    fold_obligations_versioned, phase1_lexicon, FoldRegistry, SubjectId, V1FoldImpl,
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

/// Dispatch a verb op and commit, returning the outcome as JSON.
async fn run(op: &dyn SemOsVerbOp, args: serde_json::Value, pool: &PgPool) -> serde_json::Value {
    let mut ctx = VerbExecutionContext::default();
    let mut scope = Scope::begin(pool).await;
    let out = op
        .execute(&args, &mut ctx, &mut scope)
        .await
        .expect(op.fqn());
    scope.commit().await;
    match out {
        dsl_runtime::VerbExecutionOutcome::Record(v) => v,
        other => serde_json::to_value(format!("{other:?}")).unwrap(),
    }
}

/// Dispatch a verb op without unwrapping — for tests asserting rejection.
async fn run_fallible(
    op: &dyn SemOsVerbOp,
    args: serde_json::Value,
    pool: &PgPool,
) -> anyhow::Result<serde_json::Value> {
    let mut ctx = VerbExecutionContext::default();
    let mut scope = Scope::begin(pool).await;
    let out = op.execute(&args, &mut ctx, &mut scope).await?;
    scope.commit().await;
    Ok(match out {
        dsl_runtime::VerbExecutionOutcome::Record(v) => v,
        other => serde_json::to_value(format!("{other:?}")).unwrap(),
    })
}

async fn cleanup(pool: &PgPool, subjects: &[SubjectId]) {
    for s in subjects {
        for t in [
            "kyc_intent_events",
            "kyc_subject_streams",
            "kyc_control_edge_projection",
            "kyc_obligation_projection",
            "kyc_subject_rollup_projection",
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

// ── M3.1 — Differential: freeze must match OwnershipProngStrategy (R1) ────────
//
// Mirrors the private-company fixture in `kyc_slice.rs` / `kyc_w7_oracle.rs`
// but drives it through the REAL `ubo.determination.freeze` verb (not the
// substrate directly). Before the R1 fix, freeze's proxy resolved every
// distinct edge-source as a "candidate" with no threshold and no basis —
// it would have surfaced P2 (12%, below threshold) and recorded no percentage.

#[tokio::test]
async fn m3_1_freeze_differential_matches_ownership_prong_strategy() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4()); // the case stream == entity A's own id
    let entity_b = Uuid::new_v4();
    let p1 = Uuid::new_v4();
    let p2 = Uuid::new_v4();
    let p3 = Uuid::new_v4();

    run(
        &KycSubjectRegister,
        serde_json::json!({
            "subject-id": subject.0, "is_natural_person": false,
        }),
        &pool,
    )
    .await;
    run(
        &KycSubjectClassifyStructure,
        serde_json::json!({
            "subject-id": subject.0, "structure-class": "private_company",
        }),
        &pool,
    )
    .await;
    for p in [p1, p2, p3] {
        run(
            &KycSubjectRegister,
            serde_json::json!({
                "subject-id": subject.0, "entity-id": p, "is_natural_person": true,
            }),
            &pool,
        )
        .await;
    }

    // B → A: 60%
    run(
        &UboEdgeAssertEconomicInterest,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": entity_b, "to_entity_id": subject.0,
            "percentage": 60.0,
        }),
        &pool,
    )
    .await;
    // P1 → B: 80% (effective on A: 48%)
    run(
        &UboEdgeAssertEconomicInterest,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": p1, "to_entity_id": entity_b,
            "percentage": 80.0,
        }),
        &pool,
    )
    .await;
    // P2 → B: 20% (effective on A: 12% — below 25% threshold)
    run(
        &UboEdgeAssertEconomicInterest,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": p2, "to_entity_id": entity_b,
            "percentage": 20.0,
        }),
        &pool,
    )
    .await;
    // P3 → A: 40% direct
    run(
        &UboEdgeAssertEconomicInterest,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": p3, "to_entity_id": subject.0,
            "percentage": 40.0,
        }),
        &pool,
    )
    .await;

    run(
        &UboEdgeReconcileConflict,
        serde_json::json!({ "subject-id": subject.0 }),
        &pool,
    )
    .await;
    run(
        &UboDeterminationSelectStrategy,
        serde_json::json!({
            "subject-id": subject.0, "strategy": "ownership_prong_strategy",
        }),
        &pool,
    )
    .await;

    let freeze_out = run(
        &UboDeterminationFreeze,
        serde_json::json!({
            "subject-id": subject.0, "policy-version": "v1.0",
        }),
        &pool,
    )
    .await;

    let candidates = freeze_out["candidates"]
        .as_array()
        .expect("candidates array");
    let person_ids: std::collections::BTreeSet<String> = candidates
        .iter()
        .map(|c| c["person_id"].as_str().unwrap().to_string())
        .collect();

    assert_eq!(
        candidates.len(),
        2,
        "expected exactly P1 and P3; got {candidates:?}"
    );
    assert!(
        person_ids.contains(&p1.to_string()),
        "P1 (48% effective) must resolve"
    );
    assert!(
        person_ids.contains(&p3.to_string()),
        "P3 (40% effective) must resolve"
    );
    assert!(
        !person_ids.contains(&p2.to_string()),
        "P2 (12% effective, below threshold) must NOT resolve"
    );

    for c in candidates {
        assert_eq!(
            c["prong"], "OwnershipProng",
            "K-1: basis must be recorded on every candidate"
        );
    }
    let p1_cand = candidates
        .iter()
        .find(|c| c["person_id"] == p1.to_string())
        .unwrap();
    let pct = p1_cand["effective_ownership_pct"].as_f64().unwrap();
    assert!(
        (pct - 48.0).abs() < 0.01,
        "P1 effective pct should be ~48, got {pct}"
    );

    cleanup(&pool, &[subject]).await;
}

// ── M3.2 — K-23 gate: person.approve must reject non-terminal obligations (R2) ─

#[tokio::test]
async fn m3_2_person_approve_rejects_when_obligations_not_terminal() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());

    run(
        &KycSubjectRegister,
        serde_json::json!({
            "subject-id": subject.0, "is_natural_person": true,
        }),
        &pool,
    )
    .await;
    run(
        &KycObligationCreate,
        serde_json::json!({
            "subject-id": subject.0, "role": "director", "jurisdiction": "LU",
        }),
        &pool,
    )
    .await;
    // Deliberately leave identity/screening/risk tracks Pending — no update-* calls.

    let result = run_fallible(
        &KycPersonApprove,
        serde_json::json!({
            "subject-id": subject.0,
        }),
        &pool,
    )
    .await;

    assert!(
        result.is_err(),
        "approve must be rejected while obligations are not all terminal (K-23)"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("K-23"),
        "error should name the K-23 gate; got: {msg}"
    );

    cleanup(&pool, &[subject]).await;
}

// ── M3.3 — structure_class round-trip (R3 payload-key bug) ────────────────────

#[tokio::test]
async fn m3_3_structure_class_round_trips_through_the_fold() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());

    run(
        &KycSubjectRegister,
        serde_json::json!({
            "subject-id": subject.0, "is_natural_person": false,
        }),
        &pool,
    )
    .await;
    run(
        &KycSubjectClassifyStructure,
        serde_json::json!({
            "subject-id": subject.0, "structure-class": "private_company",
        }),
        &pool,
    )
    .await;

    let fold_out = run(
        &UboDeterminationComputeFold,
        serde_json::json!({
            "subject-id": subject.0,
        }),
        &pool,
    )
    .await;

    assert_eq!(
        fold_out["structure_class"], "PrivateCompany",
        "classify-structure must set ControlState.structure_class \
         (was silently None before the R3 payload-key fix); got {fold_out:?}",
    );

    cleanup(&pool, &[subject]).await;
}

// ── M4 — control-prong strategy for fund-LP/LLP structure classes ───────────
//
// EOP-DD-KYCUBO-003 §2 Phase M4: a real `ControlProngStrategy` closing Success
// Criterion 2 (fund-LP/LLP control-prong attribution), no longer only
// `ownership_prong_strategy`. Registered in the same `freeze` dispatch match
// as `ownership_prong_strategy` (`ob_poc::domain_ops::kyc_stream_ops`).
//
// Fixture: an LP fund (subject) whose GP-statutory control edge points to a
// natural person P1 directly. `ubo.edge.assert-control` with `kind:
// gp_statutory` — the same verb `ownership_prong_strategy` fixtures use for
// `assert-economic-interest`, just the control counterpart.

#[tokio::test]
async fn m4_control_prong_strategy_resolves_gp_statutory_control() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let p1 = Uuid::new_v4();

    run(
        &KycSubjectRegister,
        serde_json::json!({
            "subject-id": subject.0, "is_natural_person": false,
        }),
        &pool,
    )
    .await;
    run(
        &KycSubjectRegister,
        serde_json::json!({
            "subject-id": subject.0, "entity-id": p1, "is_natural_person": true,
        }),
        &pool,
    )
    .await;
    run(
        &KycSubjectClassifyStructure,
        serde_json::json!({
            "subject-id": subject.0, "structure-class": "lp_fund",
        }),
        &pool,
    )
    .await;
    run(
        &UboEdgeAssertControl,
        serde_json::json!({
            "subject-id": subject.0,
            "from_entity_id": p1,
            "to_entity_id": subject.0,
            "kind": "gp_statutory",
        }),
        &pool,
    )
    .await;
    run(
        &UboEdgeReconcileConflict,
        serde_json::json!({ "subject-id": subject.0 }),
        &pool,
    )
    .await;
    run(
        &UboDeterminationSelectStrategy,
        serde_json::json!({
            "subject-id": subject.0, "strategy": "control_prong_strategy",
        }),
        &pool,
    )
    .await;

    let outcome = run(
        &UboDeterminationFreeze,
        serde_json::json!({
            "subject-id": subject.0, "policy-version": "v1.0",
        }),
        &pool,
    )
    .await;

    let candidates = outcome
        .get("candidates")
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default();
    assert_eq!(
        candidates.len(),
        1,
        "GP-statutory control edge should resolve exactly P1; got {candidates:?}"
    );
    assert_eq!(
        candidates[0].get("person_id").and_then(|v| v.as_str()),
        Some(p1.to_string().as_str()),
    );
    assert_eq!(
        candidates[0].get("prong").and_then(|v| v.as_str()),
        Some("ControlByOtherMeans"),
        "control-prong candidates must record ControlByOtherMeans, not OwnershipProng (K-1 basis)",
    );
    assert!(
        candidates[0]
            .get("effective_ownership_pct")
            .map(|v| v.is_null())
            .unwrap_or(false),
        "control has no quantum — effective_ownership_pct must be null, not a fabricated value",
    );

    cleanup(&pool, &[subject]).await;
}

// ── M3.4 (gap-documenting) — unimplemented strategies still fail loudly ─────
//
// M4 closed control_prong_strategy (above); this test now documents the
// residual boundary — a genuinely unimplemented strategy name (e.g. a
// role-based determination strategy, still not built) must fail loudly at
// freeze (K-4 spirit: never silently substitute the wrong determination
// logic), not silently fall back to a registered strategy.

#[tokio::test]
async fn m3_4_unimplemented_strategy_fails_loudly_not_silently() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());

    run(
        &KycSubjectRegister,
        serde_json::json!({
            "subject-id": subject.0, "is_natural_person": false,
        }),
        &pool,
    )
    .await;
    run(
        &KycSubjectClassifyStructure,
        serde_json::json!({
            "subject-id": subject.0, "structure-class": "lp_fund",
        }),
        &pool,
    )
    .await;
    run(
        &UboEdgeReconcileConflict,
        serde_json::json!({ "subject-id": subject.0 }),
        &pool,
    )
    .await;
    run(
        &UboDeterminationSelectStrategy,
        serde_json::json!({
            "subject-id": subject.0, "strategy": "role_based_strategy",
        }),
        &pool,
    )
    .await;

    let result = run_fallible(
        &UboDeterminationFreeze,
        serde_json::json!({
            "subject-id": subject.0, "policy-version": "v1.0",
        }),
        &pool,
    )
    .await;

    assert!(
        result.is_err(),
        "freeze must refuse an unimplemented strategy rather than silently defaulting \
         to a registered one",
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("role_based_strategy") && msg.contains("no DeterminationStrategy"),
        "error should name the missing strategy; got: {msg}",
    );

    cleanup(&pool, &[subject]).await;
}

// ── Payload-key regressions found auditing the fold-verb valid_values pattern
// (2026-07-15) — same bug class as R3 (structure_class): a YAML arg name that
// doesn't match the payload key the fold actually reads, silently dropped
// instead of erroring, because these ops pass args straight through with no
// normalize_*_payload step. ─────────────────────────────────────────────────

#[tokio::test]
async fn smo_person_id_round_trips_through_the_fold() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let smo_person = Uuid::new_v4();

    run(
        &KycSubjectRegister,
        serde_json::json!({ "subject-id": subject.0, "is_natural_person": false }),
        &pool,
    )
    .await;
    run(
        &KycSubjectClassifyStructure,
        serde_json::json!({ "subject-id": subject.0, "structure-class": "private_company" }),
        &pool,
    )
    .await;
    run(
        &UboEdgeReconcileConflict,
        serde_json::json!({ "subject-id": subject.0 }),
        &pool,
    )
    .await;
    run(
        &UboDeterminationSelectStrategy,
        serde_json::json!({ "subject-id": subject.0, "strategy": "ownership_prong_strategy" }),
        &pool,
    )
    .await;
    run(
        &UboDeterminationApplySmoFallback,
        serde_json::json!({ "subject-id": subject.0, "smo-person-id": smo_person }),
        &pool,
    )
    .await;

    let outcome = run(
        &UboDeterminationFreeze,
        serde_json::json!({ "subject-id": subject.0, "policy-version": "v1.0" }),
        &pool,
    )
    .await;

    let smo_result = outcome.get("smo_result").cloned().unwrap_or_default();
    assert_eq!(
        smo_result
            .get("Person")
            .and_then(|p| p.get("person_id"))
            .and_then(|v| v.as_str()),
        Some(smo_person.to_string().as_str()),
        "smo-person-id (kebab, YAML arg name) must reach ControlState.smo_person_id \
         (snake_case, what the fold reads) — got smo_result={smo_result:?}",
    );

    cleanup(&pool, &[subject]).await;
}

#[tokio::test]
async fn cbu_role_round_trips_through_the_obligation_fold() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());

    run(
        &KycSubjectRegister,
        serde_json::json!({ "subject-id": subject.0, "is_natural_person": false }),
        &pool,
    )
    .await;
    run(
        &KycObligationCreate,
        serde_json::json!({
            "subject-id": subject.0, "role": "shareholder", "cbu-role": "investor",
        }),
        &pool,
    )
    .await;

    let mut conn = pool.acquire().await.expect("acquire connection");
    let events = PgKycEventStore::load_events(&mut conn, subject)
        .await
        .expect("load events");
    let refs: Vec<&ob_poc_kyc_substrate::IntentEvent> = events.iter().collect();
    let mut registry = FoldRegistry::new();
    registry.register(phase1_lexicon().hash, std::sync::Arc::new(V1FoldImpl));
    let state = fold_obligations_versioned(&refs, &registry).expect("fold obligations");

    let cbu_roles: Vec<Option<String>> = state
        .obligations
        .values()
        .map(|t| t.basis.cbu_role.clone())
        .collect();
    assert!(
        cbu_roles.iter().any(|r| r.as_deref() == Some("investor")),
        "cbu-role (kebab, YAML arg name) must reach ObligationBasis.cbu_role \
         (snake_case, what the fold reads) — got cbu_roles={cbu_roles:?}",
    );

    cleanup(&pool, &[subject]).await;
}
