//! TS.2 gate tests — EOP-DD-KYCUBO-KIT-TS0 §2.2 (`fund_control_strategy`) +
//! §2.3 (`foundation_council_strategy`), ratified 2026-08-12 ACCEPT ALL.
//!
//! RED-first against the post-TS.1 tree:
//! - (a) fund end-to-end through the REAL ops: register → classify
//!   `investment_fund` → assert manager `dominant_influence` + investor
//!   `economic_interest` edges → select `fund_control_strategy` (proves the
//!   `StructureClassSupported` widening at the real append path — pre-TS.2
//!   InvestmentFund is fail-closed and select-strategy REJECTS) → resolve:
//!   manager chain only, investors excluded (economic axis stays economic),
//!   all `ControlByOtherMeans`, `effective_ownership_pct` None.
//! - (b) foundation end-to-end: register → classify `foundation` → assert
//!   `board_appointment` council edges + a stray `voting_rights` edge →
//!   select `foundation_council_strategy` → resolve: council members only,
//!   the stray `voting_rights` edge IGNORED (narrower kind filter than the
//!   full control walk — deliberate, mirrors TrustRoleStrategy's stance:
//!   if the structure genuinely mixes, classification is wrong and
//!   reclassify is legal).
//! - (c) `control_prong_strategy` behavior unchanged after the
//!   shared-traversal refactor: an explicit differential fixture — the same
//!   edge set resolves identically pre/post (plus the existing m4/w7 suites).
//! - (d) select-strategy ADMITS nominee (TS.4 fixture flip: Nominee joined
//!   the implemented set via NomineePierceStrategy — the guard set is now
//!   total; the old "still blocks" assertion inverted).
//! - (e) freeze's unknown-strategy arm still errors, message lists all
//!   implemented strategy names (TS.4 fixture fix: the exemplar moved to a
//!   never-will-exist string and the list is now 8).
//!
//! NOTE: the render.rs:102 nested-null defect was fixed 2026-08-14
//! (`render_value` omits nulls at every depth), so the final determination
//! leg of (a)/(b) now drives `kyc_ubo.decide.determination.freeze` LIVE and reads the
//! candidates + recorded strategy off the freeze outcome. Test (c) still
//! calls `ControlProngStrategy.resolve` directly on purpose — it pins
//! strategy-level differential behavior, not the freeze path.
//!
//! DB/fixture pattern mirrors `kyc_ts1_trust.rs`.

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::{TransactionScope, VerbExecutionContext};
use ob_poc::domain_ops::kyc_stream_ops::{
    KycSubjectClassifyStructure, KycSubjectPlace,
    UboDeterminationFreeze, UboEdgeConnect,
};
use ob_poc_kyc_substrate::{
    fold_control_versioned, assembly_lexicon, DeterminationStrategy, FoldRegistry, IntentEvent,
    SubjectId, V1FoldImpl,
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

/// Register subject + natural persons, then classify with `structure_class`.
async fn setup_subject(pool: &PgPool, subject: SubjectId, persons: &[Uuid], class: &str) {
    run(
        &KycSubjectPlace,
        serde_json::json!({ "subject-id": subject.0, "is_natural_person": false, "entity-type": "private_limited_company" }),
        pool,
    )
    .await;
    for p in persons {
        run(
            &KycSubjectPlace,
            serde_json::json!({
                "subject-id": subject.0, "entity-id": p, "is_natural_person": true, "entity-type": "natural_person",
            }),
            pool,
        )
        .await;
    }
    run(
        &KycSubjectClassifyStructure,
        serde_json::json!({ "subject-id": subject.0, "structure-class": class }),
        pool,
    )
    .await;
}

/// Drive the REAL freeze op, assert the recorded strategy name on the
/// outcome, and return the candidates array (render nested-null defect
/// fixed 2026-08-14 — freeze is drivable live for pct-less candidates).
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
        "structure_class must derive {expected_strategy} (TS.6 P2: strategy is derived, \
         not separately asserted) — freeze dispatches to it and records it on the \
         outcome; got {outcome:?}"
    );
    outcome
        .get("candidates")
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default()
}

// ── (a) fund_control_strategy end-to-end ────────────────────────────────────
//
// Manager (ManCo/AIFM/GP-analog) asserted as `dominant_influence` (§2.2: no
// new EdgeKind — the management relationship IS the control edge); investors
// as `economic_interest`. The strategy walks the control axis only: the
// manager chain resolves, investors are excluded even at 90%.

#[tokio::test]
async fn a_fund_control_strategy_resolves_manager_and_excludes_investors() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let manager = Uuid::new_v4();
    let investor_big = Uuid::new_v4(); // 90% economic — still NOT a control candidate
    let investor_small = Uuid::new_v4();

    setup_subject(
        &pool,
        subject,
        &[manager, investor_big, investor_small],
        "investment_fund",
    )
    .await;

    run(
        &UboEdgeConnect,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": manager, "to_entity_id": subject.0,
            "kind": "dominant_influence",
        }),
        &pool,
    )
    .await;
    for (inv, pct) in [(investor_big, 90.0), (investor_small, 10.0)] {
        run(
            &UboEdgeConnect,
            serde_json::json!({
                "subject-id": subject.0, "from_entity_id": inv, "to_entity_id": subject.0,
                "kind": "economic_interest", "percentage": pct,
            }),
            &pool,
        )
        .await;
    }


    // The gate widening proven at the REAL op: freeze on an
    // InvestmentFund-classified subject is now ADMITTED (StructureClassSupported)
    // — pre-TS.2 this call fails-closed. `freeze_candidates` below succeeding
    // IS the proof (a precondition failure surfaces as an error there); no
    // separate probe verb exists since `compute-fold` retired (TS.6 P2).
    let candidates = freeze_candidates(&pool, subject, "fund_control_strategy").await;
    let person_ids: std::collections::BTreeSet<String> = candidates
        .iter()
        .filter_map(|c| c.get("person_id").and_then(|v| v.as_str()).map(String::from))
        .collect();

    assert_eq!(
        candidates.len(),
        1,
        "expected exactly the manager chain; got {candidates:#?}"
    );
    assert!(
        person_ids.contains(&manager.to_string()),
        "the fund manager (dominant_influence) must be the control candidate (§2.2)"
    );
    for (inv, name) in [(investor_big, "90% investor"), (investor_small, "10% investor")] {
        assert!(
            !person_ids.contains(&inv.to_string()),
            "{name} must NOT be a fund_control_strategy candidate — investor \
             economic_interest edges stay on the economic axis (§2.2)"
        );
    }
    for c in &candidates {
        assert_eq!(
            c.get("prong").and_then(|v| v.as_str()),
            Some("ControlByOtherMeans"),
            "K-1 basis: every fund-control candidate is ControlByOtherMeans"
        );
        assert!(
            c.get("effective_ownership_pct")
                .map(|v| v.is_null())
                .unwrap_or(false),
            "fund control has no quantum — effective_ownership_pct must be null"
        );
    }

    cleanup(&pool, &[subject]).await;
}

// ── (b) foundation_council_strategy end-to-end ──────────────────────────────
//
// Council members asserted as `board_appointment`; a stray `voting_rights`
// edge on the Foundation subject must be IGNORED by this strategy (narrower
// kind filter — §2.3: a foundation has no owners by construction; if the
// structure genuinely mixes, classification is wrong and reclassify is
// legal).

#[tokio::test]
async fn b_foundation_council_strategy_resolves_council_and_ignores_voting_rights() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let council_a = Uuid::new_v4();
    let council_b = Uuid::new_v4();
    let stray_voter = Uuid::new_v4();

    setup_subject(
        &pool,
        subject,
        &[council_a, council_b, stray_voter],
        "foundation",
    )
    .await;

    for member in [council_a, council_b] {
        run(
            &UboEdgeConnect,
            serde_json::json!({
                "subject-id": subject.0, "from_entity_id": member, "to_entity_id": subject.0,
                "kind": "board_appointment",
            }),
            &pool,
        )
        .await;
    }
    run(
        &UboEdgeConnect,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": stray_voter, "to_entity_id": subject.0,
            "kind": "voting_rights",
        }),
        &pool,
    )
    .await;

    let candidates = freeze_candidates(&pool, subject, "foundation_council_strategy").await;
    let person_ids: std::collections::BTreeSet<String> = candidates
        .iter()
        .filter_map(|c| c.get("person_id").and_then(|v| v.as_str()).map(String::from))
        .collect();

    assert_eq!(
        candidates.len(),
        2,
        "expected exactly the two council members; got {candidates:#?}"
    );
    for (member, name) in [(council_a, "council member A"), (council_b, "council member B")] {
        assert!(
            person_ids.contains(&member.to_string()),
            "{name} (board_appointment) must be a control candidate (§2.3)"
        );
    }
    assert!(
        !person_ids.contains(&stray_voter.to_string()),
        "a stray voting_rights edge on a Foundation subject must be IGNORED by \
         foundation_council_strategy (deliberate kind-filter, §2.3)"
    );
    for c in &candidates {
        assert_eq!(
            c.get("prong").and_then(|v| v.as_str()),
            Some("ControlByOtherMeans"),
            "K-1 basis: every council candidate is ControlByOtherMeans"
        );
        assert!(
            c.get("effective_ownership_pct")
                .map(|v| v.is_null())
                .unwrap_or(false),
            "foundation control has no quantum — effective_ownership_pct must be null"
        );
    }

    cleanup(&pool, &[subject]).await;
}

// ── (c) control_prong_strategy differential — refactor behavior-preserving ──
//
// Pure fold-level (no DB): a mixed control-kind edge set (voting_rights,
// board_appointment, gp_statutory, dominant_influence; a two-hop chain
// through an intermediate entity; an excluded nominee edge; an economic
// edge) must resolve to exactly the same candidate set through
// `ControlProngStrategy` after the shared-traversal refactor as the
// pre-refactor impl produced. The existing m4/w7 suites also cover this;
// this fixture makes the differential explicit inside the TS.2 gate file.

#[test]
fn c_control_prong_strategy_behavior_unchanged_after_shared_helper_refactor() {
    use chrono::{TimeZone, Utc};
    use ob_poc_kyc_substrate::{AuthorityRef, Principal, TargetBinding};

    let subject = SubjectId(Uuid::new_v4());
    let subject_entity = subject.0;
    let direct_voter = Uuid::new_v4(); // natural person, voting_rights → candidate
    let board_holder = Uuid::new_v4(); // natural person, board_appointment → candidate
    let intermediate = Uuid::new_v4(); // legal entity, gp_statutory → traversed
    let gp_principal = Uuid::new_v4(); // natural person behind intermediate → candidate
    let nominee = Uuid::new_v4(); // nominee kind → excluded (K-8)
    let economic_only = Uuid::new_v4(); // economic axis → excluded

    let as_of = Utc.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
    let lexicon_hash = assembly_lexicon().hash;
    let mut reg = FoldRegistry::new();
    reg.register(lexicon_hash, std::sync::Arc::new(V1FoldImpl));

    let mk = |verb: &str, payload: serde_json::Value| {
        IntentEvent::new(
            subject,
            verb,
            Principal::test_analyst(),
            AuthorityRef("ts2-test".into()),
            TargetBinding::for_subject(subject),
            payload,
            as_of,
        )
        .with_lexicon_hash(lexicon_hash)
    };
    let edge = |from: Uuid, to: Uuid, kind: &str| {
        mk(
            "kyc_ubo.assert.edge.control",
            serde_json::json!({ "from_entity_id": from, "to_entity_id": to, "kind": kind }),
        )
    };

    let register = |entity: Uuid| {
        mk(
            "kyc_ubo.assert.subject.register",
            serde_json::json!({ "entity_id": entity, "is_natural_person": true, "entity-type": "natural_person" }),
        )
    };

    let events = [
        register(direct_voter),
        register(board_holder),
        register(gp_principal),
        register(nominee),
        register(economic_only),
        mk(
            "kyc_ubo.assert.subject.structure-class",
            serde_json::json!({ "structure_class": "llp", "entity_id": subject_entity }),
        ),
        edge(direct_voter, subject_entity, "voting_rights"),
        edge(board_holder, subject_entity, "board_appointment"),
        edge(intermediate, subject_entity, "gp_statutory"),
        edge(gp_principal, intermediate, "dominant_influence"),
        edge(nominee, subject_entity, "nominee"),
        mk(
            "kyc_ubo.assert.edge.economic-interest",
            serde_json::json!({
                "from_entity_id": economic_only, "to_entity_id": subject_entity,
                "percentage": 40.0,
            }),
        ),
    ];
    let refs: Vec<&IntentEvent> = events.iter().collect();
    let control = fold_control_versioned(&refs, &reg).expect("fold ok");

    let persons: std::collections::BTreeSet<ob_poc_kyc_substrate::PersonId> =
        [direct_voter, board_holder, gp_principal, nominee, economic_only]
            .into_iter()
            .map(ob_poc_kyc_substrate::PersonId)
            .collect();

    let candidates = ob_poc_kyc_substrate::ControlProngStrategy.resolve(
        &control,
        ob_poc_kyc_substrate::EntityId(subject_entity),
        &persons,
        25.0,
    );
    let person_ids: std::collections::BTreeSet<Uuid> =
        candidates.iter().map(|c| c.person_id.0).collect();

    // The exact pre-refactor semantics, pinned:
    let expected: std::collections::BTreeSet<Uuid> =
        [direct_voter, board_holder, gp_principal].into_iter().collect();
    assert_eq!(
        person_ids, expected,
        "ControlProngStrategy must resolve exactly {{direct voter, board holder, \
         GP principal via the intermediate}} — nominee excluded (K-8), economic \
         axis excluded — identically to the pre-refactor impl; got {candidates:#?}"
    );
    for c in &candidates {
        assert_eq!(c.prong, ob_poc_kyc_substrate::Prong::ControlByOtherMeans);
        assert!(c.effective_ownership_pct.is_none());
    }
    // The chain through the intermediate is preserved (path recording intact).
    let gp = candidates
        .iter()
        .find(|c| c.person_id.0 == gp_principal)
        .expect("gp principal resolved");
    assert!(
        gp.ownership_chain.contains(&ob_poc_kyc_substrate::EntityId(intermediate)),
        "the two-hop chain must record the intermediate entity; got {:?}",
        gp.ownership_chain
    );
}

// ── (d) nominee ADMITTED post-TS.4 (the guard set went total) ───────────────

#[tokio::test]
async fn d_select_strategy_admits_nominee_after_ts4() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());

    // TS.4 fixture flip (was `d_select_strategy_still_blocks_nominee_after_
    // widening`): Nominee joined the implemented set (NomineePierceStrategy),
    // so the `[ReconciledProjection, StructureClassSupported]` pair (formerly
    // also carried by `compute-fold`, retired TS.6 P2) on a Nominee-classified
    // subject is now ADMITTED — the old assertion inverted. Probed here via
    // `apply-smo-fallback`, which declares the identical precondition pair and
    // (unlike `freeze`) needs no candidate-producing edges to reach it — this
    // fixture has none. The fail-closed floor for unknown/garbage class
    // strings is pinned in kyc_t61_studs.rs and kyc_pack_closure.rs.
    setup_subject(&pool, subject, &[], "nominee").await;

    // StructureClassSupported must ADMIT a Nominee-classified subject
    // (widened to the total 11-variant set at TS.4). Previously probed via
    // `apply-smo-fallback`, which declared the identical precondition pair —
    // retired TS.6 §5 (SMO is PULLED on exhaustion by the traversal, never
    // asserted). `freeze` is now the only verb carrying that pair, so it is
    // the probe: it will still error here (K-5 — no candidates and nothing
    // for the SMO pull to find), but that is a DOWNSTREAM refusal reached
    // only AFTER the preconditions admitted. The proof is the absence of the
    // StructureClassSupported refusal text, not success.
    let result = run_fallible(
        &UboDeterminationFreeze,
        serde_json::json!({ "subject-id": subject.0, "policy-version": "v1.0" }),
        &pool,
    )
    .await;
    let msg = match &result {
        Ok(_) => String::new(),
        Err(e) => e.to_string(),
    };
    assert!(
        !msg.contains("has no implemented determination strategy yet"),
        "StructureClassSupported must ADMIT a Nominee-classified subject \
         post-TS.4 (widened to the total set); got: {msg}"
    );

    cleanup(&pool, &[subject]).await;
}

// ── (e) freeze unknown-strategy arm — RETIRED (TS.6 P2) ─────────────────────
//
// `e_freeze_rejects_unknown_strategy_listing_all_implemented` drove
// `select-strategy` with a `"no_such_strategy"` string to prove freeze's
// catch-all arm. That entry point is gone: the strategy is now DERIVED
// from `structure_class`, a TOTAL function over all 11 `StructureClass`
// variants (pinned by
// `kyc_pack_closure.rs::precondition_and_strategy_coverage_is_exactly_known`),
// so there is no governed way left to hand freeze an arbitrary strategy
// name — see the identical note on `kyc_ts1_trust.rs`'s deleted
// `f_freeze_still_rejects_unknown_strategy_after_trust_widening`.
