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
//! FENCED-DEFECT NOTE (same as `kyc_ts1_trust.rs` (d)/(e)): the final leg
//! cannot drive `ubo.determination.freeze` itself — freeze's event payload
//! embeds the candidates, and every ControlByOtherMeans candidate carries
//! `effective_ownership_pct: null`, which trips the PRE-EXISTING
//! render.rs:102 nested-null defect (the exact reason
//! `m4_control_prong_strategy_resolves_gp_statutory_control` and
//! `coverage_ubo_determination_freeze` are fenced pre-existing reds). That
//! defect stays fenced in TS.2, so candidates are resolved via the exact
//! strategy instance freeze dispatches to (arm proven by test (e) +
//! `implemented_class_split_matches_strategy_arms`), over the REAL DB-loaded
//! stream + the same `fold_control_versioned` / `natural_persons_from_events`
//! / `find_subject_entity` composition the freeze op uses. Upgrade to drive
//! freeze directly once the render defect is fixed.
//!
//! DB/fixture pattern mirrors `kyc_ts1_trust.rs`.

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::{TransactionScope, VerbExecutionContext};
use ob_poc::domain_ops::kyc_stream_ops::{
    KycSubjectClassifyStructure, KycSubjectRegister, UboDeterminationFreeze,
    UboDeterminationSelectStrategy, UboEdgeAssertControl, UboEdgeAssertEconomicInterest,
    UboEdgeReconcileConflict,
};
use ob_poc_kyc_substrate::{
    fold_control_versioned, phase1_lexicon, DeterminationStrategy, FoldRegistry, IntentEvent,
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

/// Register subject + natural persons, then classify with `structure_class`.
async fn setup_subject(pool: &PgPool, subject: SubjectId, persons: &[Uuid], class: &str) {
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
        serde_json::json!({ "subject-id": subject.0, "structure-class": class }),
        pool,
    )
    .await;
}

/// Load the subject's REAL stream from the DB, fold it, assert the recorded
/// strategy name, and resolve via the given strategy instance — the exact
/// composition the freeze op uses (see the FENCED-DEFECT NOTE above).
async fn resolve_via_strategy(
    pool: &PgPool,
    subject: SubjectId,
    expected_strategy: &str,
    strategy: &dyn DeterminationStrategy,
) -> Vec<ob_poc_kyc_substrate::ProngCandidate> {
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
        Some(expected_strategy),
        "select-strategy (real op) must have recorded {expected_strategy}"
    );
    let subject_entity =
        ob_poc_kyc_substrate::find_subject_entity(&refs).expect("classify recorded entity");
    let persons = ob_poc_kyc_substrate::natural_persons_from_events(&refs);
    strategy.resolve(&control, subject_entity, &persons, 25.0)
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
        &UboEdgeAssertControl,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": manager, "to_entity_id": subject.0,
            "kind": "dominant_influence",
        }),
        &pool,
    )
    .await;
    for (inv, pct) in [(investor_big, 90.0), (investor_small, 10.0)] {
        run(
            &UboEdgeAssertEconomicInterest,
            serde_json::json!({
                "subject-id": subject.0, "from_entity_id": inv, "to_entity_id": subject.0,
                "percentage": pct,
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

    // The gate widening proven at the REAL op: select-strategy on an
    // InvestmentFund-classified subject is now ADMITTED
    // (StructureClassSupported) — pre-TS.2 this call fails-closed.
    let select = run_fallible(
        &UboDeterminationSelectStrategy,
        serde_json::json!({ "subject-id": subject.0, "strategy": "fund_control_strategy" }),
        &pool,
    )
    .await;
    assert!(
        select.is_ok(),
        "select-strategy on an InvestmentFund-classified subject must be admitted \
         post-TS.2 (StructureClassSupported widened): {select:?}"
    );

    let candidates = resolve_via_strategy(
        &pool,
        subject,
        "fund_control_strategy",
        &ob_poc_kyc_substrate::FundControlStrategy,
    )
    .await;
    let person_ids: std::collections::BTreeSet<String> = candidates
        .iter()
        .map(|c| c.person_id.0.to_string())
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
            c.prong,
            ob_poc_kyc_substrate::Prong::ControlByOtherMeans,
            "K-1 basis: every fund-control candidate is ControlByOtherMeans"
        );
        assert!(
            c.effective_ownership_pct.is_none(),
            "fund control has no quantum — effective_ownership_pct must be None"
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
            &UboEdgeAssertControl,
            serde_json::json!({
                "subject-id": subject.0, "from_entity_id": member, "to_entity_id": subject.0,
                "kind": "board_appointment",
            }),
            &pool,
        )
        .await;
    }
    run(
        &UboEdgeAssertControl,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": stray_voter, "to_entity_id": subject.0,
            "kind": "voting_rights",
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
    let select = run_fallible(
        &UboDeterminationSelectStrategy,
        serde_json::json!({ "subject-id": subject.0, "strategy": "foundation_council_strategy" }),
        &pool,
    )
    .await;
    assert!(
        select.is_ok(),
        "select-strategy on a Foundation-classified subject must be admitted \
         post-TS.2 (StructureClassSupported widened): {select:?}"
    );

    let candidates = resolve_via_strategy(
        &pool,
        subject,
        "foundation_council_strategy",
        &ob_poc_kyc_substrate::FoundationCouncilStrategy,
    )
    .await;
    let person_ids: std::collections::BTreeSet<String> = candidates
        .iter()
        .map(|c| c.person_id.0.to_string())
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
            c.prong,
            ob_poc_kyc_substrate::Prong::ControlByOtherMeans,
            "K-1 basis: every council candidate is ControlByOtherMeans"
        );
        assert!(
            c.effective_ownership_pct.is_none(),
            "foundation control has no quantum — effective_ownership_pct must be None"
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
    let lexicon_hash = phase1_lexicon().hash;
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
            "ubo.edge.assert-control",
            serde_json::json!({ "from_entity_id": from, "to_entity_id": to, "kind": kind }),
        )
    };

    let register = |entity: Uuid| {
        mk(
            "kyc.subject.register",
            serde_json::json!({ "entity_id": entity, "is_natural_person": true }),
        )
    };

    let events = [
        register(direct_voter),
        register(board_holder),
        register(gp_principal),
        register(nominee),
        register(economic_only),
        mk(
            "kyc.subject.classify-structure",
            serde_json::json!({ "structure_class": "llp", "entity_id": subject_entity }),
        ),
        edge(direct_voter, subject_entity, "voting_rights"),
        edge(board_holder, subject_entity, "board_appointment"),
        edge(intermediate, subject_entity, "gp_statutory"),
        edge(gp_principal, intermediate, "dominant_influence"),
        edge(nominee, subject_entity, "nominee"),
        mk(
            "ubo.edge.assert-economic-interest",
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
    // so select-strategy on a Nominee-classified subject is now ADMITTED —
    // the old assertion inverted. The fail-closed floor for unknown/garbage
    // class strings is pinned in kyc_t61_studs.rs and kyc_pack_closure.rs.
    setup_subject(&pool, subject, &[], "nominee").await;

    let result = run_fallible(
        &UboDeterminationSelectStrategy,
        serde_json::json!({ "subject-id": subject.0, "strategy": "nominee_pierce_strategy" }),
        &pool,
    )
    .await;
    assert!(
        result.is_ok(),
        "select-strategy on a Nominee-classified subject must be ADMITTED \
         post-TS.4 (StructureClassSupported widened to the total set): {result:?}"
    );

    cleanup(&pool, &[subject]).await;
}

// ── (e) freeze unknown-strategy arm errors, listing all implemented ─────────

#[tokio::test]
async fn e_freeze_rejects_unknown_strategy_listing_all_implemented() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let member = Uuid::new_v4();

    setup_subject(&pool, subject, &[member], "foundation").await;
    run(
        &UboEdgeAssertControl,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": member, "to_entity_id": subject.0,
            "kind": "board_appointment",
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
    // A strategy name with no arm behind it — TS.4 fixture fix:
    // nominee_pierce_strategy gained a real dispatch arm, so the exemplar
    // is now a never-will-exist string.
    run(
        &UboDeterminationSelectStrategy,
        serde_json::json!({ "subject-id": subject.0, "strategy": "no_such_strategy" }),
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
        msg.contains("no_such_strategy") && msg.contains("no DeterminationStrategy"),
        "error should name the missing strategy; got: {msg}"
    );
    for implemented in [
        "ownership_prong_strategy",
        "control_prong_strategy",
        "trust_role_strategy",
        "fund_control_strategy",
        "foundation_council_strategy",
        "state_owned_strategy",
        "cooperative_member_strategy",
        "nominee_pierce_strategy",
    ] {
        assert!(
            msg.contains(implemented),
            "the rejection must list all 8 implemented strategies (expected \
             {implemented} in: {msg})"
        );
    }

    cleanup(&pool, &[subject]).await;
}
