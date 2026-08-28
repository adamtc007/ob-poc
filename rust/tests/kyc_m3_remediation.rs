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
    KycSubjectPlace, UboDeterminationFreeze, UboEdgeConnect,
};
use ob_poc_kyc_store::PgKycEventStore;
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
        .unwrap_or_else(|error| panic!("{}: {error}", op.fqn()));
    scope.commit().await;
    match out {
        dsl_runtime::VerbExecutionOutcome::Record(v) => v,
        other => serde_json::to_value(format!("{other:?}")).unwrap(),
    }
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

// ── M3.1 — Differential: freeze must match OwnershipProngStrategy (R1) ────────
//
// Mirrors the private-company fixture in `kyc_slice.rs` / `kyc_w7_oracle.rs`
// but drives it through the REAL `kyc_ubo.decide.determination.freeze` verb (not the
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

    // EOP-DD-UBO-DISPATCH-001 T4-close (2026-08-28): the `classify-structure`
    // call this fixture used to make after `place` is deleted —
    // `structure-class` is retired, and `place`'s own `entity-type:
    // "private_limited_company"` already drives dispatch to
    // `ownership_prong_strategy` (§2), so the classify call was pure setup,
    // redundant with the fact `place` already asserted the dispatch key.
    run(
        &KycSubjectPlace,
        serde_json::json!({
            "subject-id": subject.0, "is_natural_person": false, "entity-type": "private_limited_company",
        }),
        &pool,
    )
    .await;
    for p in [p1, p2, p3] {
        run(
            &KycSubjectPlace,
            serde_json::json!({
                "subject-id": subject.0, "entity-id": p, "is_natural_person": true, "entity-type": "natural_person",
            }),
            &pool,
        )
        .await;
    }

    // B → A: 60%
    run(
        &UboEdgeConnect,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": entity_b, "to_entity_id": subject.0,
            "kind": "economic_interest", "percentage": 60.0,
        }),
        &pool,
    )
    .await;
    // P1 → B: 80% (effective on A: 48%)
    run(
        &UboEdgeConnect,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": p1, "to_entity_id": entity_b,
            "kind": "economic_interest", "percentage": 80.0,
        }),
        &pool,
    )
    .await;
    // P2 → B: 20% (effective on A: 12% — below 25% threshold)
    run(
        &UboEdgeConnect,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": p2, "to_entity_id": entity_b,
            "kind": "economic_interest", "percentage": 20.0,
        }),
        &pool,
    )
    .await;
    // P3 → A: 40% direct
    run(
        &UboEdgeConnect,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": p3, "to_entity_id": subject.0,
            "kind": "economic_interest", "percentage": 40.0,
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

// ── M3.2 — RETIRED (EOP-DD-KYCUBO-D2.0 §5, 2026-08-22) ─────────────────────
//
// `m3_2_person_approve_rejects_when_obligations_not_terminal` tested the
// obligation-fold-based K-23 gate ("reject while an obligation track is
// still Pending"). `kyc_ubo.assert.obligation.creation` — the only writer
// of a new `ObligationTracks` entry — is dissolved, so that mechanism no
// longer exists to test: `decide.approve`'s K-23 gate now cites an
// `EvaluationRun`'s work list (see `tests/kyc_verb_coverage.rs`'s
// `decide_cites_a_run`), which is trivially empty against today's empty
// check catalogue (D2.0 §7 Q2, out of scope) — this exact test, run
// unmodified against the new gate, would now assert the OPPOSITE of what
// actually happens (approve succeeds, not fails).

// ── M3.3 — entity-type round-trip through the type-registry fold ───────────
//
// REWRITTEN (EOP-DD-UBO-DISPATCH-001 T4-close, 2026-08-28) — was
// `m3_3_structure_class_round_trips_through_the_fold`, proving R3's fix:
// `kyc_ubo.assert.subject.structure-class`'s payload key reached
// `ControlState.structure_class` intact (it had silently stayed `None`
// before the fix, because the fold read a different key than the verb
// wrote). That mechanism is gone — `structure-class` is retired, and
// `ControlState.structure_class` is now a permanently-`None` dead field
// (nothing can ever set it again); asserting on it would test a retired
// mechanism, not prove anything live. T4 moved the SAME defect class onto
// `place`'s `entity-type` argument, since entity-type is now the SOLE key
// both geometry AND strategy dispatch read (T4 §1) — a `place`/fold
// payload-key mismatch here would be today's R3: dispatch would silently
// break, the same way R3's classify call once did. This rewrite proves the
// live round-trip: `entity-type` asserted through `place` reaches
// `fold_type_registry` intact, and `dispatch_for_entity_type` resolves the
// correct strategy from it — the T4 equivalent of what M3.3 always
// checked, per the `ec2_conflicting_edges_fail_without_reconcile`
// precedent (rewrite, don't delete, when a rule genuinely changed).

#[tokio::test]
async fn m3_3_entity_type_round_trips_through_the_type_registry_fold() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());

    run(
        &KycSubjectPlace,
        serde_json::json!({
            "subject-id": subject.0, "is_natural_person": false, "entity-type": "private_limited_company",
        }),
        &pool,
    )
    .await;

    let mut conn = pool.acquire().await.expect("acquire connection");
    let events = PgKycEventStore::load_events(&mut conn, subject)
        .await
        .expect("load events");
    let refs: Vec<&ob_poc_kyc_substrate::IntentEvent> = events.iter().collect();
    let subject_entity =
        ob_poc_kyc_substrate::find_subject_entity(&refs).expect("subject entity");
    let type_registry = ob_poc_kyc_substrate::fold_type_registry(&refs);

    assert_eq!(
        type_registry.type_of(subject_entity),
        Some(ob_poc_kyc_substrate::EntityType::PrivateLimitedCompany),
        "place's entity-type argument must reach fold_type_registry intact \
         (was silently dropped/mismatched before the R3-class fix this \
         mechanism replaces); got {:?}",
        type_registry.type_of(subject_entity),
    );
    assert_eq!(
        match ob_poc_kyc_substrate::dispatch_for_entity_type(
            &ob_poc_kyc_substrate::EntityType::PrivateLimitedCompany
        ) {
            ob_poc_kyc_substrate::DeterminationDispatch::Strategy(name) => Some(name),
            ob_poc_kyc_substrate::DeterminationDispatch::NotADeterminationSubject => None,
        },
        Some("ownership_prong_strategy"),
        "PrivateLimitedCompany must dispatch to ownership_prong_strategy (§2)"
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
// Fixture: a limited partnership (subject) whose GP-statutory control edge
// points to a natural person P1 directly. `kyc_ubo.assert.edge.connect` with
// `kind: gp_statutory` — the same merged verb `ownership_prong_strategy`
// fixtures use with `kind: economic_interest`, just the control counterpart.
//
// EOP-DD-UBO-DISPATCH-001 T4-close (2026-08-28): subject entity-type was
// `lp_fund` — under the OLD singular `StructureClass::LimitedPartnershipFund`
// bucket this dispatched to `control_prong_strategy` (this test's own
// name), but T4 §2 SPLIT that bucket: `EntityType::LpFund` now dispatches
// to `fund_control_strategy` (`kyc_t4_dispatch.rs::
// lp_fund_dispatches_to_fund_control_not_control_prong`), while
// `EntityType::LimitedPartnership` is the type that still dispatches to
// `control_prong_strategy` (§2). Keeping `lp_fund` here would silently
// re-target this test at `fund_control_strategy`'s pivot/evidence
// machinery (TS.4 §2 Ruling 2f) instead of proving what its name and
// assertions (`prong: ControlByOtherMeans`, no pivot) actually claim —
// `limited_partnership` is the entity-type that keeps this test's original
// intent intact. `LimitedPartnership`'s TS.1 target_permits admits
// `GpDesignation` (`gp_statutory`'s pipe), same as before.

#[tokio::test]
async fn m4_control_prong_strategy_resolves_gp_statutory_control() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let p1 = Uuid::new_v4();

    run(
        &KycSubjectPlace,
        serde_json::json!({
            "subject-id": subject.0, "is_natural_person": false, "entity-type": "limited_partnership",
        }),
        &pool,
    )
    .await;
    run(
        &KycSubjectPlace,
        serde_json::json!({
            "subject-id": subject.0, "entity-id": p1, "is_natural_person": true, "entity-type": "natural_person",
        }),
        &pool,
    )
    .await;
    run(
        &UboEdgeConnect,
        serde_json::json!({
            "subject-id": subject.0,
            "from_entity_id": p1,
            "to_entity_id": subject.0,
            "kind": "gp_statutory",
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

// ── M3.4 — RETIRED (pre-existing latent break, closed by TS.4 totality) ────
//
// `m3_4_unimplemented_strategy_fails_loudly_not_silently` classified the
// subject as `"lp_fund"` (`StructureClass::LimitedPartnershipFund`) and
// expected freeze's error to name a fictitious `"role_based_strategy"`.
// This premise was ALREADY unreachable before TS.6: the test never called
// `select-strategy`, so even pre-TS.6 `control.selected_strategy` was
// `None` and freeze failed with "no strategy selected (K-4 precondition)",
// never containing `"role_based_strategy"` — this test has been red since
// it was written (an independent, pre-existing latent bug, not a TS.6
// regression; surfaced only because TS.6's real-DB regression run reached
// it with a message assertion, whereas earlier runs evidently didn't gate
// on it failing for the wrong reason). TS.4 then closed the underlying gap
// for real: `LimitedPartnershipFund` now dispatches to the implemented
// `control_prong_strategy` (M4), and `IMPLEMENTED_STRATEGY_CLASSES` is
// TOTAL (11/11) — there is no structure class left, named or garbage, that
// reaches freeze's `other => Err(...)` catch-all through a real
// `classify-structure` call (the fail-closed floor for garbage/unknown
// wire strings, which fold to `structure_class: None`, is pinned in
// `kyc_t61_studs.rs`/`kyc_pack_closure.rs` instead). Retired rather than
// fixed, per the same reasoning as the deleted
// `f_freeze_rejects_unknown_strategy_listing_all_eight`-family tests
// elsewhere in this KYC test suite.

// ── Payload-key regressions found auditing the fold-verb valid_values pattern
// (2026-07-15) — same bug class as R3 (structure_class): a YAML arg name that
// doesn't match the payload key the fold actually reads, silently dropped
// instead of erroring, because these ops pass args straight through with no
// normalize_*_payload step. ─────────────────────────────────────────────────

// `smo_person_id_round_trips_through_the_fold` RETIRED (TS.6 §5, 2026-08-22).
// It proved `normalize_smo_fallback_payload` mapped the YAML kebab arg
// `smo-person-id` onto the snake key `smo_person_id` the fold reads — a real
// defect when found. Both the verb (`ubo.determination.apply-smo-fallback`)
// and its normalizer are now retired: SMO is PULLED on exhaustion by the
// traversal (TS.3 §4a) rather than asserted, so nothing writes
// `ControlState.smo_person_id` any more and there is no payload to normalize.
// The sibling round-trip gates below (cbu-role, structure-class) still cover
// the defect CLASS for the args that remain.

// `cbu_role_round_trips_through_the_obligation_fold` RETIRED (D2.0 §5,
// 2026-08-22): it proved `kyc_ubo.assert.obligation.creation`'s kebab→snake
// payload normalization (`cbu-role`→`cbu_role`) reached `ObligationBasis`.
// `creation` is dissolved, so there is no live write path left to guard —
// same disposition as the already-retired `smo_person_id` sibling gate,
// above. `structure_class_round_trips_through_the_control_fold` (if present
// elsewhere in this suite) still covers the defect CLASS for a surviving verb.
