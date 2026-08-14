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
    KycSubjectClassifyStructure, KycSubjectRegister, UboDeterminationFreeze,
    UboDeterminationSelectStrategy, UboEdgeAssertControl, UboEdgeReconcileConflict,
};
use ob_poc_kyc_substrate::{
    fold_control_versioned, phase1_lexicon, AuthorityRef, EdgeKind, FoldRegistry, IntentEvent,
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
    let lexicon_hash = phase1_lexicon().hash;
    let mut reg = FoldRegistry::new();
    reg.register(lexicon_hash, std::sync::Arc::new(V1FoldImpl));

    let mk = |kind: &str| {
        IntentEvent::new(
            subject,
            "ubo.edge.assert-control",
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
// FENCED-DEFECT NOTE: the final leg cannot drive `ubo.determination.freeze`
// itself — freeze's event payload embeds the candidates, and every
// ControlByOtherMeans candidate carries `effective_ownership_pct: null`,
// which trips the PRE-EXISTING render.rs:102 nested-null defect (the exact
// reason `m4_control_prong_strategy_resolves_gp_statutory_control` and
// `coverage_ubo_determination_freeze` are fenced pre-existing reds — any
// pct-less candidate set panics freeze's render today). That defect is
// fenced (not fixed) in TS.1, so candidates are resolved here via the exact
// `TrustRoleStrategy` instance freeze dispatches to (arm proven by test (f)
// + `implemented_class_split_matches_strategy_arms`), over the REAL
// DB-loaded stream + the same `fold_control_versioned` /
// `natural_persons_from_events` / `find_subject_entity` composition the
// freeze op uses. Upgrade these two tests to drive freeze directly once the
// render defect is fixed.

async fn resolve_via_selected_strategy(
    pool: &PgPool,
    subject: SubjectId,
) -> Vec<ob_poc_kyc_substrate::ProngCandidate> {
    use ob_poc_kyc_substrate::DeterminationStrategy;

    let mut conn = pool.acquire().await.expect("acquire connection");
    let events = ob_poc_kyc_store::PgKycEventStore::load_events(&mut conn, subject)
        .await
        .expect("load events");
    let refs: Vec<&IntentEvent> = events.iter().collect();
    let mut reg = FoldRegistry::new();
    reg.register(phase1_lexicon().hash, std::sync::Arc::new(V1FoldImpl));
    let control = fold_control_versioned(&refs, &reg).expect("fold ok");
    assert_eq!(
        control.selected_strategy.as_deref(),
        Some("trust_role_strategy"),
        "select-strategy (real op) must have recorded trust_role_strategy"
    );
    let subject_entity =
        ob_poc_kyc_substrate::find_subject_entity(&refs).expect("classify recorded entity");
    let persons = ob_poc_kyc_substrate::natural_persons_from_events(&refs);
    ob_poc_kyc_substrate::TrustRoleStrategy.resolve(&control, subject_entity, &persons, 25.0)
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
    // The gate widening proven at the REAL op: select-strategy on a
    // Trust-classified subject is now ADMITTED (StructureClassSupported).
    run(
        &UboDeterminationSelectStrategy,
        serde_json::json!({ "subject-id": subject.0, "strategy": "trust_role_strategy" }),
        &pool,
    )
    .await;

    let candidates = resolve_via_selected_strategy(&pool, subject).await;
    let person_ids: std::collections::BTreeSet<String> = candidates
        .iter()
        .map(|c| c.person_id.0.to_string())
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
            c.prong,
            ob_poc_kyc_substrate::Prong::ControlByOtherMeans,
            "K-1 basis: every trust-role candidate is ControlByOtherMeans"
        );
        assert!(
            c.effective_ownership_pct.is_none(),
            "trust control has no quantum — effective_ownership_pct must be None"
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
    run(
        &UboDeterminationSelectStrategy,
        serde_json::json!({ "subject-id": subject.0, "strategy": "trust_role_strategy" }),
        &pool,
    )
    .await;

    // Same fenced-defect note as (d): resolved via the exact strategy freeze
    // dispatches to, over the real DB-loaded stream.
    let candidates = resolve_via_selected_strategy(&pool, subject).await;
    let person_ids: std::collections::BTreeSet<String> = candidates
        .iter()
        .map(|c| c.person_id.0.to_string())
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

// ── (f) unknown strategy still fails loudly post-widening ───────────────────

#[tokio::test]
async fn f_freeze_still_rejects_unknown_strategy_after_trust_widening() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let trustee = Uuid::new_v4();

    setup_trust_subject(&pool, subject, &[trustee]).await;
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
    // TS.2 fixture fix: foundation_council_strategy gained a real dispatch
    // arm (FoundationCouncilStrategy), so the unknown-strategy exemplar
    // becomes state_owned_strategy (unimplemented until TS.3 — moves again
    // then).
    run(
        &UboDeterminationSelectStrategy,
        serde_json::json!({ "subject-id": subject.0, "strategy": "state_owned_strategy" }),
        &pool,
    )
    .await;

    let result = run_fallible(
        &UboDeterminationFreeze,
        serde_json::json!({ "subject-id": subject.0, "policy-version": "v1.0" }),
        &pool,
    )
    .await;
    assert!(
        result.is_err(),
        "freeze must refuse an unimplemented strategy rather than silently \
         substituting a registered one"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("state_owned_strategy") && msg.contains("no DeterminationStrategy"),
        "error should name the missing strategy; got: {msg}"
    );

    cleanup(&pool, &[subject]).await;
}
