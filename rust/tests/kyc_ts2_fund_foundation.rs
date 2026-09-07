//! TS.2 gate tests — EOP-DD-KYCUBO-KIT-TS0 §2.2 (`fund_control_strategy`) +
//! §2.3 (`foundation_council_strategy`), ratified 2026-08-12 ACCEPT ALL.
//!
//! RED-first against the post-TS.1 tree:
//! - (a) fund end-to-end through the REAL ops: `place` as `lp_fund` →
//!   assert GP `gp_statutory` + investor `economic_interest` edges →
//!   freeze dispatches to `fund_control_strategy` (T4-close, 2026-08-28:
//!   entity-type is the dispatch key; was `dominant_influence` + a
//!   separate `classify` call, pre-T4) → resolve: GP chain only, investors
//!   excluded (economic axis stays economic), all `ControlByOtherMeans`,
//!   `effective_ownership_pct` None.
//! - (b) foundation end-to-end: `place` as `foundation` → assert
//!   `trust_trustee` council edges + a stray `trust_beneficiary` edge →
//!   freeze dispatches to `foundation_council_strategy` → resolve: council
//!   members only, the beneficiary edge IGNORED (per-role admission,
//!   corrected T4-close 2026-08-28 per Adam's Foundation ruling — the
//!   strategy now walks `TrustRole`-kind edges, the same filter
//!   `TrustRoleStrategy` uses, since a foundation council governs under
//!   fiduciary roles, not board seats; a foundation council/founder/
//!   beneficiary maps onto trustee/settlor/beneficiary 1:1).
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
    KycSubjectPlace,
    UboDeterminationFreeze, UboEdgeAttachEvidence, UboEdgeConnect,
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

/// Register subject + natural persons with a real dispatchable EntityType.
///
/// EOP-DD-UBO-DISPATCH-001 T4-close (2026-08-28): `class` used to be the
/// separately-asserted `structure-class`, decoupled from `place`'s
/// `entity-type` (only geometry cared the two agreed) — `entity-type` was
/// hardcoded `private_limited_company` here regardless of `class`.
/// `structure-class` is retired; entity-type is now the SOLE dispatch key,
/// so `class` must resolve to the EntityType each fixture's scenario
/// actually needs. `investment_fund`→`oeic_icvc` (`fund_control_strategy`),
/// `foundation`→`foundation` (`foundation_council_strategy`) — direct §2
/// matches for this file's (a)/(b) scenarios.
async fn setup_subject(pool: &PgPool, subject: SubjectId, persons: &[Uuid], class: &str) {
    let entity_type = match class {
        "investment_fund" => "oeic_icvc",
        "foundation" => "foundation",
        other => other,
    };
    run(
        &KycSubjectPlace,
        serde_json::json!({ "subject-id": subject.0, "entity-type": entity_type }),
        pool,
    )
    .await;
    for p in persons {
        run(
            &KycSubjectPlace,
            serde_json::json!({
                "subject-id": subject.0, "entity-id": p, "entity-type": "natural_person",
            }),
            pool,
        )
        .await;
    }
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
// GP (the LP fund's governing-mandate holder) asserted as `gp_statutory`
// (§2.2's governing-mandate relationship IS the control edge, TS.4 §2
// Ruling A — `ManagementMandate` OR `GpStatutory`, "basis, not the label
// 'ManCo'"); investors as `economic_interest`. The strategy walks the
// control axis only: the GP chain resolves, investors are excluded even
// at 90%.
//
// EOP-DD-UBO-DISPATCH-001 T4-close (2026-08-28): was `dominant_influence`
// from a bare natural person onto an `OeicIcvc` — geometrically impossible
// even before this rewrite (`OeicIcvc` target_permits admits
// `BoardAppointment`/`ManagementMandate`/`UnitIssuance`/
// `PooledAssetContainment`, not `ContractualControl`). The natural next fix
// (swap to `management_mandate`, still on `OeicIcvc`) is ALSO geometrically
// impossible: §2a pipe 7 is ruled "corporate only... never a natural
// person" (2026-08-19), but `fund_pivot_resolve` (TS.4 §2 Ruling A) only
// stops at the pivot when the pivot IS a natural person — for a corporate
// pivot it re-anchors and keeps walking, so a `ManagementMandate`-sourced
// corporate manager can never itself be the resolved candidate; something
// must be layered on top of it, changing what the test proves. `GpStatutory`
// is the OTHER ratified governing-mandate kind, its pipe (`GpDesignation`)
// permits a NATURAL source (§2a pipe 3: "corporates and natural persons
// only"), and `LpFund`'s target_permits admits it — a GP who is a natural
// person is both realistic (the common case for a small LP) and lets the
// pivot resolve directly, matching this test's original "the manager IS
// the candidate" shape without re-anchoring through a second entity.

#[tokio::test]
async fn a_fund_control_strategy_resolves_manager_and_excludes_investors() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let manager = Uuid::new_v4();
    let investor_big = Uuid::new_v4(); // 90% economic — still NOT a control candidate
    let investor_small = Uuid::new_v4();

    run(
        &KycSubjectPlace,
        serde_json::json!({ "subject-id": subject.0, "entity-type": "lp_fund" }),
        &pool,
    )
    .await;
    for p in [manager, investor_big, investor_small] {
        run(
            &KycSubjectPlace,
            serde_json::json!({
                "subject-id": subject.0, "entity-id": p, "entity-type": "natural_person",
            }),
            &pool,
        )
        .await;
    }

    let connect_out = run(
        &UboEdgeConnect,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": manager, "to_entity_id": subject.0,
            "kind": "gp_statutory",
        }),
        &pool,
    )
    .await;
    // TS.4 §2 Ruling 2f (CTN-2e "record freely, conclude carefully"): a
    // governing-mandate pivot edge must be evidenced before freeze will
    // conclude through it.
    let mandate_edge = connect_out["edge_id"].as_str().expect("edge_id").to_string();
    run(
        &UboEdgeAttachEvidence,
        serde_json::json!({
            "subject-id": subject.0, "edge-id": mandate_edge,
            "kind": "contract", "source": "test fixture", "date": "2026-08-28",
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
        "the fund GP (gp_statutory) must be the control candidate (§2.2)"
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
// Council members asserted as `trust_trustee`; a stray `trust_beneficiary`
// edge on the Foundation subject must be IGNORED by this strategy (narrower
// per-role admission — §2.3: a foundation has no owners by construction; if
// the structure genuinely mixes, classification is wrong and reclassify is
// legal).
//
// UN-DEFERRED (EOP-DD-UBO-DISPATCH-001 Foundation ruling, Adam 2026-08-28,
// recorded in EOP-STATE-KYCUBO-D1 §5p/§5q). The T4-close tranche found
// `foundation_council_strategy` walking `BoardAppointment`/
// `DominantInfluence` while `Foundation`'s TS.1 geometry admits only
// `TrusteePowers`/`ReservedPowers`/`BeneficiaryEntitlement` — neither edge
// kind the strategy needed could ever be asserted onto a real Foundation
// through the governed path, so this test could only ever pass on a
// hand-built fixture. Adam ruled: the GEOMETRY is right, the STRATEGY is
// wrong — a foundation council governs under the charter the way trustees
// govern under a deed (5AMLD treats foundations as trust-like);
// `BoardAppointment` is a company mechanism, never the right fit.
// `FoundationCouncilStrategy` now walks `TrustRole`-kind edges (the SAME
// filter `TrustRoleStrategy` uses, `determination.rs`), so this fixture
// asserts through the REAL governed path — `trust_trustee` for council
// members (maps to `Pipe::TrusteePowers`, TS.1-permitted for Foundation)
// and `trust_beneficiary` for the excluded stray edge (maps to
// `Pipe::BeneficiaryEntitlement`, also TS.1-permitted, but never a
// candidate under `TrustRoleStrategy::edge_qualifies`'s per-role rule,
// reused directly by `FoundationCouncilStrategy`) — rather than a
// hand-built `ControlState`. A test that only passed on a fixture is what
// hid the geometry/strategy disagreement for as long as it did.
#[tokio::test]
async fn b_foundation_council_strategy_resolves_council_and_ignores_beneficiary() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let council_a = Uuid::new_v4();
    let council_b = Uuid::new_v4();
    let stray_beneficiary = Uuid::new_v4();

    setup_subject(
        &pool,
        subject,
        &[council_a, council_b, stray_beneficiary],
        "foundation",
    )
    .await;

    for member in [council_a, council_b] {
        run(
            &UboEdgeConnect,
            serde_json::json!({
                "subject-id": subject.0, "from_entity_id": member, "to_entity_id": subject.0,
                "kind": "trust_trustee",
            }),
            &pool,
        )
        .await;
    }
    run(
        &UboEdgeConnect,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": stray_beneficiary, "to_entity_id": subject.0,
            "kind": "trust_beneficiary",
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
            "{name} (trust_trustee) must be a control candidate (§2.3)"
        );
    }
    assert!(
        !person_ids.contains(&stray_beneficiary.to_string()),
        "a beneficiary edge on a Foundation subject must be IGNORED by \
         foundation_council_strategy (per-role admission: beneficiary is \
         never a candidate, §2.3)"
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
    // T3 (§3.1/§3.2) merged `assert.edge.control` + `assert.edge.economic-interest`
    // into one verb, `connect` — `kind` alone distinguishes them now.
    let edge = |from: Uuid, to: Uuid, kind: &str| {
        mk(
            "kyc_ubo.assert.edge.connect",
            serde_json::json!({ "from_entity_id": from, "to_entity_id": to, "kind": kind }),
        )
    };

    // `place` absorbs register (+ structure-class + assert-type, T2/T6.1(a)).
    // Only `ControlState.registered_entity_ids` matters to this fixture —
    // `ControlProngStrategy::resolve` takes no `TypeRegistryState`/structure-class
    // argument — so `entity-type` is omitted.
    let register = |entity: Uuid| {
        mk(
            "kyc_ubo.assert.subject.place",
            serde_json::json!({ "entity_id": entity }),
        )
    };

    let events = [
        register(direct_voter),
        register(board_holder),
        register(gp_principal),
        register(nominee),
        register(economic_only),
        edge(direct_voter, subject_entity, "voting_rights"),
        edge(board_holder, subject_entity, "board_appointment"),
        edge(intermediate, subject_entity, "gp_statutory"),
        edge(gp_principal, intermediate, "dominant_influence"),
        edge(nominee, subject_entity, "nominee"),
        edge(economic_only, subject_entity, "economic_interest"),
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

// ── (d) a real, supported EntityType is admitted past the type gate;
//        K-5 (empty candidates), not EntityTypeSupportsStrategy, is what
//        refuses an empty structure ─────────────────────────────────────

// REWRITTEN (EOP-DD-UBO-DISPATCH-001 T4-close, 2026-08-28) — was
// `d_select_strategy_admits_nominee_after_ts4`, proving `StructureClassSupported`
// admitted a Nominee-classified subject (TS.4 widened the guard to total,
// 11/11 classes). That mechanism is gone: `structure-class` is retired, and
// no `EntityType` dispatches to `nominee_pierce_strategy` any longer (§2 —
// "where nominee_pierce_strategy went: nowhere, deliberately"; nominees are
// pierced mid-traversal under whichever strategy the SUBJECT's real type
// resolves to, not by classifying the subject itself as a nominee). The
// PROPERTY this test proved still holds and is reasserted here: a subject
// whose type resolves to a live strategy is admitted PAST
// `EntityTypeSupportsStrategy` even with zero candidate-producing edges —
// K-5 (empty determination) is the refusal that fires, not the type gate.
// `llp` (`control_prong_strategy`, §2) stands in for the old `nominee`
// fixture value; the fail-closed floor for an UNSUPPORTED type is pinned in
// `kyc_t4_dispatch.rs::unmapped_type_refuses_freeze_by_name`, not here.
#[tokio::test]
async fn d_a_supported_empty_structure_fails_at_k5_not_the_type_gate() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());

    setup_subject(&pool, subject, &[], "llp").await;

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
    assert!(result.is_err(), "an empty structure with zero candidates must still refuse (K-5)");
    assert!(
        msg.contains("(K-5)") && msg.contains("determination would be silent"),
        "the refusal must be K-5 (empty determination), not the EntityTypeSupportsStrategy \
         type gate — `llp` is a real, live-dispatchable type; got: {msg}"
    );
    assert!(
        !msg.contains("is not a determination subject") && !msg.contains("no entity type known"),
        "EntityTypeSupportsStrategy must ADMIT a subject whose type resolves to a live \
         strategy — the type gate must not be what refuses here; got: {msg}"
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
