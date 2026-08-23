//! TS.1 gate tests — EOP-DD-KYCUBO-KIT-TS0 §1 (wire-vocabulary ruling) +
//! §2.1 (`trust_role_strategy`), ratified 2026-08-12 ACCEPT ALL.
//!
//! RED-first against the pre-TS.1 tree:
//! - (a) the four trust wire values fold to their distinct `TrustRole`
//!   sub-kinds and derive DISTINCT edge ids for the same (from, to) pair —
//!   pre-TS.1 they all collapse to `DominantInfluence` (R4 fact 3).
//! - (b) `NoDuplicateActiveEdge` admits settlor+trustee between the same
//!   pair (distinct kinds) but blocks a second trustee — pre-TS.1 the
//!   collapse makes the second role a duplicate of `DominantInfluence`.
//! - (c) the op-normalizer (§1b) hard-errors on an unknown OR absent `kind`,
//!   listing the 11 valid wire values — pre-TS.1 both silently resolve to
//!   `DominantInfluence` and append.
//! - (d)/(e) `trust_role_strategy` end-to-end through the REAL ops:
//!   trustee + protector always candidates; settlor included unless the edge
//!   proves irrevocability (`trust-revocable: false` — fail-closed toward
//!   inclusion); beneficiary NEVER a candidate (economic axis only). All
//!   candidates `ControlByOtherMeans`, `effective_ownership_pct` null.
//! - (f) freeze still fails loudly on an unknown strategy name post-widening.
//!
//! DB/fixture pattern mirrors `kyc_t62_studs.rs` / `kyc_m3_remediation.rs`.

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use chrono::{TimeZone, Utc};
use dsl_runtime::{TransactionScope, VerbExecutionContext};
use ob_poc::domain_ops::kyc_stream_ops::{
    KycSubjectClassifyStructure, KycSubjectRegister, UboDeterminationFreeze, UboEdgeAssertControl,
    UboEdgeReconcileConflict,
};
use ob_poc_kyc_substrate::{
    fold_control_versioned, assembly_lexicon, AuthorityRef, EdgeKind, FoldRegistry, IntentEvent,
    Principal, SubjectId, TargetBinding, TrustRoleKind, V1FoldImpl,
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
/// Fresh `VerbExecutionContext` per call — `execution_id` seeds the
/// idempotency key (B3); reuse would dedupe follow-on calls.
async fn run(op: &dyn SemOsVerbOp, args: serde_json::Value, pool: &PgPool) -> serde_json::Value {
    let mut ctx = VerbExecutionContext::default();
    let mut scope = Scope::begin(pool).await;
    let out = op
        .execute(&args, &mut ctx, &mut scope)
        .await
        .unwrap_or_else(|error| panic!("{}: {error}", op.fqn()));
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

// ── (a) four wire values → distinct TrustRole sub-kinds + distinct edge ids ──
//
// Pure fold-level (no DB): the fold's `edge_kind_from_payload` + derived-id
// scheme. No `edge_id` in the payload — exercising the fold's own
// `Uuid::new_v5("control:{from}:{to}:{kind:?}")` derivation, which pre-TS.1
// collided all four roles onto one `DominantInfluence` id (R4 fact 3a).

#[test]
fn a_trust_wire_values_fold_to_distinct_sub_kinds_and_edge_ids() {
    let subject = SubjectId(Uuid::new_v4());
    let from = Uuid::new_v4();
    let to = Uuid::new_v4();
    let as_of = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let lexicon_hash = assembly_lexicon().hash;
    let mut reg = FoldRegistry::new();
    reg.register(lexicon_hash, std::sync::Arc::new(V1FoldImpl));

    let mk = |kind: &str| {
        IntentEvent::new(
            subject,
            "kyc_ubo.assert.edge.control",
            Principal::test_analyst(),
            AuthorityRef("ts1-test".into()),
            TargetBinding::for_subject(subject),
            serde_json::json!({
                "from_entity_id": from,
                "to_entity_id": to,
                "kind": kind,
            }),
            as_of,
        )
        .with_lexicon_hash(lexicon_hash)
    };

    let events = [
        mk("trust_settlor"),
        mk("trust_trustee"),
        mk("trust_protector"),
        mk("trust_beneficiary"),
    ];
    let refs: Vec<&IntentEvent> = events.iter().collect();
    let state = fold_control_versioned(&refs, &reg).expect("fold ok");

    assert_eq!(
        state.edges.len(),
        4,
        "four trust roles between the same (from,to) pair must derive four \
         DISTINCT edge ids (the sub-kind sits inside the {{:?}} id render) — \
         pre-TS.1 they collapsed onto one DominantInfluence id; got {:#?}",
        state.edges
    );

    let expected = [
        EdgeKind::TrustRole(TrustRoleKind::Settlor),
        EdgeKind::TrustRole(TrustRoleKind::Trustee),
        EdgeKind::TrustRole(TrustRoleKind::Protector),
        EdgeKind::TrustRole(TrustRoleKind::Beneficiary),
    ];
    for kind in expected {
        assert!(
            state.edges.values().any(|e| e.kind == kind),
            "wire value for {kind:?} must fold to exactly that sub-kind, \
             never DominantInfluence; got kinds {:?}",
            state.edges.values().map(|e| &e.kind).collect::<Vec<_>>()
        );
    }
}

// ── (b) NoDuplicateActiveEdge distinguishes trust roles ─────────────────────

#[tokio::test]
async fn b_settlor_and_trustee_coexist_but_second_trustee_is_blocked() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let from = Uuid::new_v4();

    run(
        &KycSubjectRegister,
        serde_json::json!({ "subject-id": subject.0, "is_natural_person": false }),
        &pool,
    )
    .await;

    // Same (from, to) pair: settlor then trustee — distinct kinds, both admitted.
    run(
        &UboEdgeAssertControl,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": from, "to_entity_id": subject.0,
            "kind": "trust_settlor",
        }),
        &pool,
    )
    .await;
    let trustee = run_fallible(
        &UboEdgeAssertControl,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": from, "to_entity_id": subject.0,
            "kind": "trust_trustee",
        }),
        &pool,
    )
    .await;
    assert!(
        trustee.is_ok(),
        "a trustee edge between the same (from,to) pair as an existing settlor edge must be \
         admitted — the roles are distinct kinds (R4 fact 3 fixed): {trustee:?}"
    );

    // A SECOND trustee between the same pair is a genuine duplicate.
    let dup = run_fallible(
        &UboEdgeAssertControl,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": from, "to_entity_id": subject.0,
            "kind": "trust_trustee",
        }),
        &pool,
    )
    .await;
    assert!(
        dup.is_err(),
        "a second active trustee edge for the same (from,to) must be blocked \
         (NoDuplicateActiveEdge, K-13)"
    );

    cleanup(&pool, &[subject]).await;
}

// ── (c) §1b — the op-normalizer kills the silent catch-all ──────────────────

#[tokio::test]
async fn c_normalizer_rejects_unknown_and_absent_kind_listing_wire_values() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());

    run(
        &KycSubjectRegister,
        serde_json::json!({ "subject-id": subject.0, "is_natural_person": false }),
        &pool,
    )
    .await;

    // Unknown kind — the exact "trust_role" string the pre-TS.1 doc warned
    // silently collapsed to DominantInfluence.
    let unknown = run_fallible(
        &UboEdgeAssertControl,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": Uuid::new_v4(),
            "to_entity_id": subject.0, "kind": "trust_role",
        }),
        &pool,
    )
    .await;
    assert!(
        unknown.is_err(),
        "an unrecognized kind string must be rejected at the op-normalizer \
         (§1b fail-closed), never silently appended as DominantInfluence"
    );
    let msg = unknown.unwrap_err().to_string();
    for wire in ["trust_settlor", "trust_trustee", "trust_protector", "trust_beneficiary", "voting_rights", "dominant_influence"] {
        assert!(
            msg.contains(wire),
            "the rejection must list the valid wire values (expected {wire} in: {msg})"
        );
    }

    // Absent kind — same rejection.
    let absent = run_fallible(
        &UboEdgeAssertControl,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": Uuid::new_v4(),
            "to_entity_id": subject.0,
        }),
        &pool,
    )
    .await;
    assert!(
        absent.is_err(),
        "an absent kind must be rejected at the op-normalizer (§1b), never \
         silently appended as DominantInfluence"
    );

    cleanup(&pool, &[subject]).await;
}

// ── (d) trust_role_strategy end-to-end ──────────────────────────────────────
//
// Register subject + four natural persons → classify `trust` → assert one
// edge per role → reconcile → select `trust_role_strategy` through the REAL
// op (passes `StructureClassSupported` now Trust is in the implemented set —
// the TS.1 gate widening proven end-to-end) → resolve candidates.
// Candidates must be exactly {settlor, trustee, protector} (settlor included
// because revocability is unproven — fail-closed toward inclusion);
// beneficiary excluded (economic axis only). All `ControlByOtherMeans`,
// `effective_ownership_pct` None.
//
// NOTE: the render.rs:102 nested-null defect was fixed 2026-08-14
// (`render_value` now omits nulls at every depth), so these tests drive
// `kyc_ubo.decide.determination.freeze` LIVE and read the candidates + recorded
// strategy off the freeze outcome.

/// Drive the REAL freeze op, assert the recorded strategy name on the
/// outcome, and return the candidates array.
async fn freeze_candidates(
    pool: &PgPool,
    subject: SubjectId,
    expected_strategy: &str,
) -> Vec<serde_json::Value> {
    let outcome = run(
        &UboDeterminationFreeze,
        serde_json::json!({ "subject-id": subject.0, "policy-version": "v1.0" }),
        pool,
    )
    .await;
    assert_eq!(
        outcome.get("strategy").and_then(|v| v.as_str()),
        Some(expected_strategy),
        "select-strategy (real op) must have recorded {expected_strategy} — freeze \
         dispatches to it and records it on the outcome; got {outcome:?}"
    );
    outcome
        .get("candidates")
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default()
}

async fn setup_trust_subject(
    pool: &PgPool,
    subject: SubjectId,
    persons: &[Uuid],
) {
    run(
        &KycSubjectRegister,
        serde_json::json!({ "subject-id": subject.0, "is_natural_person": false }),
        pool,
    )
    .await;
    for p in persons {
        run(
            &KycSubjectRegister,
            serde_json::json!({
                "subject-id": subject.0, "entity-id": p, "is_natural_person": true,
            }),
            pool,
        )
        .await;
    }
    run(
        &KycSubjectClassifyStructure,
        serde_json::json!({ "subject-id": subject.0, "structure-class": "trust" }),
        pool,
    )
    .await;
}

#[tokio::test]
async fn d_trust_role_strategy_resolves_trustee_protector_and_revocable_settlor() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let settlor = Uuid::new_v4();
    let trustee = Uuid::new_v4();
    let protector = Uuid::new_v4();
    let beneficiary = Uuid::new_v4();

    setup_trust_subject(&pool, subject, &[settlor, trustee, protector, beneficiary]).await;

    for (p, kind) in [
        (settlor, "trust_settlor"),
        (trustee, "trust_trustee"),
        (protector, "trust_protector"),
        (beneficiary, "trust_beneficiary"),
    ] {
        run(
            &UboEdgeAssertControl,
            serde_json::json!({
                "subject-id": subject.0, "from_entity_id": p, "to_entity_id": subject.0,
                "kind": kind,
            }),
            &pool,
        )
        .await;
    }

    run(
        &UboEdgeReconcileConflict,
        serde_json::json!({ "subject-id": subject.0 }),
        &pool,
    )
    .await;

    let candidates = freeze_candidates(&pool, subject, "trust_role_strategy").await;
    let person_ids: std::collections::BTreeSet<String> = candidates
        .iter()
        .filter_map(|c| c.get("person_id").and_then(|v| v.as_str()).map(String::from))
        .collect();

    assert_eq!(
        candidates.len(),
        3,
        "expected exactly settlor+trustee+protector; got {candidates:#?}"
    );
    for (p, name) in [(settlor, "settlor"), (trustee, "trustee"), (protector, "protector")] {
        assert!(
            person_ids.contains(&p.to_string()),
            "{name} must be a control candidate (settlor: revocability unproven ⇒ included, \
             fail-closed toward inclusion)"
        );
    }
    assert!(
        !person_ids.contains(&beneficiary.to_string()),
        "beneficiary must NEVER be a trust_role_strategy candidate — economic axis only \
         (ratified §2.1)"
    );

    for c in &candidates {
        assert_eq!(
            c.get("prong").and_then(|v| v.as_str()),
            Some("ControlByOtherMeans"),
            "K-1 basis: every trust-role candidate is ControlByOtherMeans"
        );
        assert!(
            c.get("effective_ownership_pct")
                .map(|v| v.is_null())
                .unwrap_or(false),
            "trust control has no quantum — effective_ownership_pct must be null"
        );
    }

    cleanup(&pool, &[subject]).await;
}

// ── (e) settlor excluded only when PROVEN irrevocable ───────────────────────

#[tokio::test]
async fn e_settlor_excluded_when_trust_revocable_is_false() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let settlor = Uuid::new_v4();
    let trustee = Uuid::new_v4();

    setup_trust_subject(&pool, subject, &[settlor, trustee]).await;

    // Settlor edge carrying the irrevocability proof — the kebab-case YAML
    // arg `trust-revocable` must reach the fold's snake_case payload key
    // (the exact edge-id kebab/snake defect class, normalized at the op).
    run(
        &UboEdgeAssertControl,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": settlor, "to_entity_id": subject.0,
            "kind": "trust_settlor", "trust-revocable": false,
        }),
        &pool,
    )
    .await;
    run(
        &UboEdgeAssertControl,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": trustee, "to_entity_id": subject.0,
            "kind": "trust_trustee",
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

    // Final leg drives the REAL freeze op (render nested-null defect fixed
    // 2026-08-14).
    let candidates = freeze_candidates(&pool, subject, "trust_role_strategy").await;
    let person_ids: std::collections::BTreeSet<String> = candidates
        .iter()
        .filter_map(|c| c.get("person_id").and_then(|v| v.as_str()).map(String::from))
        .collect();
    assert!(
        person_ids.contains(&trustee.to_string()),
        "trustee always resolves"
    );
    assert!(
        !person_ids.contains(&settlor.to_string()),
        "settlor must be EXCLUDED when the edge proves irrevocability \
         (trust_revocable == Some(false)); got {candidates:#?}"
    );

    cleanup(&pool, &[subject]).await;
}

// ── (f) unknown strategy — RETIRED (TS.6 P2) ────────────────────────────────
//
// `f_freeze_still_rejects_unknown_strategy_after_trust_widening` exercised
// `select-strategy` to inject an arbitrary `"no_such_strategy"` string, then
// proved freeze refused it. That entry point is gone: the strategy is now
// DERIVED from `structure_class` (`strategy_for_structure_class`), a TOTAL
// function over all 11 `StructureClass` variants pinned by
// `kyc_pack_closure.rs::precondition_and_strategy_coverage_is_exactly_known`
// — there is no governed way left to hand freeze an arbitrary strategy
// name. `UboDeterminationFreeze`'s `other => Err(...)` match arm is
// unreachable through the real write path today; it remains as
// defense-in-depth against a future non-total edit to
// `strategy_for_structure_class`, not as live, verb-reachable capability —
// same status as `IMPLEMENTED_STRATEGY_CLASSES` before TS.0-TS.4 closed it.
