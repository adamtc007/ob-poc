//! TS.4 gate tests — EOP-DD-KYCUBO-KIT-TS0 §2.6 (`kyc_ubo.assert.edge.nominee-piercing` +
//! `nominee_pierce_strategy`), ratified 2026-08-12 ACCEPT ALL. TS.4 = K-8:
//! the LAST TS tranche — the only class needing a NEW verb, exercised with
//! full kit citizenship (the K-G7 reintroduction discipline).
//!
//! RED-first against the post-TS.3 tree (T4-close, 2026-08-28: entity-type
//! is the dispatch key now; `structure-class` is retired, and no
//! `EntityType` dispatches to `nominee_pierce_strategy` — §2 "nowhere,
//! deliberately"):
//! - (a) pierce end-to-end through the REAL ops: `place` as `llc_us` →
//!   assert a nominee edge → pierce with the disclosed nominator → the
//!   fold shows BOTH effects of the ONE governed event: the target nominee
//!   edge Superseded with `superseded_by` = the pierce event, and a NEW
//!   edge from the nominator with the underlying kind and `pierced_from`
//!   provenance (K-13 supersede-never-contradict; K-35 traceability).
//! - (b) pierce REFUSES a non-nominee target edge (no precondition
//!   primitive expresses "edge is of kind X" — the check is op-layer,
//!   fail-closed, per the §2.6 checklist note).
//! - (b2) the pierce normalizer REFUSES `kind: nominee` (a pierce cannot
//!   produce another nominee edge — fail-closed) and any unknown kind
//!   (the TS.1 §1b wire-normalizer discipline extended to the new verb).
//! - (c) pierce REFUSES an inactive (superseded) target (EdgeActive stud,
//!   matrix row 3/4 vocabulary, enforced at the real append path) and a
//!   non-existent target edge.
//! - (d) freeze HARD-ERRORS while an unpierced nominee edge remains active
//!   — `resolve()` cannot signal error (trait returns `Vec<ProngCandidate>`),
//!   so the fail-closed scan lives at the freeze dispatch site
//!   (kyc_stream_ops.rs), UNCONDITIONALLY, before strategy dispatch (TS.4
//!   §3 Ruling B — reachable under ANY strategy, not only when the subject
//!   itself was classified `Nominee`).
//! - (e) post-pierce, freeze dispatches by the subject's real type
//!   (`llc_us` → `control_prong_strategy`) and resolves the NOMINATOR chain
//!   (the underlying structure), never the nominee (K-8's whole point) —
//!   the delegation `NomineePierceStrategy` used to perform explicitly now
//!   happens implicitly, because the subject's own type was the real
//!   determinant all along.
//! - (f) freeze's unknown-strategy arm — RETIRED (TS.6 P2), comment-only.
//!
//! NOTE: the render.rs:102 nested-null defect was fixed 2026-08-14
//! (`render_value` omits nulls at every depth), so test (e)'s final
//! determination leg now drives `kyc_ubo.decide.determination.freeze` LIVE to SUCCESS
//! and reads the candidates + recorded strategy off the freeze outcome.
//! Tests (d)/(f) drive the real freeze op's error paths as before.
//!
//! DB/fixture pattern mirrors `kyc_ts3_state_owned_cooperative.rs`.

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::{TransactionScope, VerbExecutionContext};
use ob_poc::domain_ops::kyc_stream_ops::{
    KycSubjectPlace,
    UboDeterminationFreeze, UboEdgeConnect, UboEdgeDisconnect,
};
use ob_poc_kyc_substrate::{
    fold_control_versioned, assembly_lexicon, EdgeKind, EdgeStatus, FoldRegistry, IntentEvent,
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
/// EOP-DD-UBO-DISPATCH-001 T4-close (2026-08-28): every fixture in this
/// file used the class label `"nominee"`, which is retired along with
/// `structure-class` — no `EntityType` dispatches to `nominee_pierce_strategy`
/// any longer (§2: "where nominee_pierce_strategy went: nowhere,
/// deliberately"; `NomineePierceStrategy` is "a thin delegate to the
/// control-prong traversal" once every nominee edge is pierced). `llc_us`
/// is a `control_prong_strategy` type (§2) whose TS.1 target_permits
/// admits `VotingShares` (this file's `nominee`/pierced-from edges are all
/// `voting_rights`) — `private_limited_company` (the old hardcoded
/// placeholder, ignoring `class` entirely pre-T4-close) dispatches to
/// `ownership_prong_strategy` instead, which would silently invalidate
/// test (e)'s strategy assertion. The nominee/nominator entities
/// themselves are deliberately left unregistered/registered as before —
/// `EdgeKind::Nominee`'s pipe (`NomineeHolding`) is absent from every
/// target's TS.1 permitted-pipe row by ratified design (`geometry.rs`
/// module doc), so a nominee edge's `from` (the on-paper holder) must stay
/// type-unregistered for `Unevaluable`/admit — the fixtures already do
/// this by never placing `nominee`, only `nominator`.
async fn setup_subject(pool: &PgPool, subject: SubjectId, persons: &[Uuid]) {
    run(
        &KycSubjectPlace,
        serde_json::json!({ "subject-id": subject.0, "entity-type": "llc_us" }),
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

/// Fold the subject's REAL DB stream (the same composition the ops use).
async fn fold_subject(pool: &PgPool, subject: SubjectId) -> ob_poc_kyc_substrate::ControlState {
    let mut conn = pool.acquire().await.expect("acquire connection");
    let events = ob_poc_kyc_store::PgKycEventStore::load_events(&mut conn, subject)
        .await
        .expect("load events");
    let refs: Vec<&IntentEvent> = events.iter().collect();
    let mut reg = FoldRegistry::new();
    reg.register(assembly_lexicon().hash, std::sync::Arc::new(V1FoldImpl));
    fold_control_versioned(&refs, &reg).expect("fold ok")
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
        "select-strategy (real op) must have recorded {expected_strategy} — freeze \
         dispatches to it and records it on the outcome; got {outcome:?}"
    );
    outcome
        .get("candidates")
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default()
}

// ── (a) pierce end-to-end — supersede + reassert in ONE governed event ──────
//
// §2.6: (a) the target nominee edge is superseded (`superseded_by` = the
// pierce event); (b) a new control edge from the nominator is asserted with
// the underlying kind and `pierced_from` provenance.

#[tokio::test]
async fn a_pierce_supersedes_nominee_and_asserts_nominator_edge() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let nominee = Uuid::new_v4(); // the on-paper holder
    let nominator = Uuid::new_v4(); // the disclosed true holder

    setup_subject(&pool, subject, &[nominator]).await;

    // §8 Q3: connect refuses a caller-supplied edge id — capture the minted one.
    let connect_out = run(
        &UboEdgeConnect,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": nominee, "to_entity_id": subject.0,
            "kind": "nominee",
        }),
        &pool,
    )
    .await;
    let nominee_edge = Uuid::parse_str(connect_out["edge_id"].as_str().expect("edge_id"))
        .expect("edge_id must be a valid UUID");

    // TS.6 P2 (K-G7): `kyc_ubo.assert.edge.nominee-piercing` is now a macro composing
    // `kyc_ubo.assert.edge.connect` (pierced-from) + `kyc_ubo.assert.edge.disconnect` — TWO
    // governed events, not one; the macro's two steps commit atomically
    // under the Sequencer's one-scope-per-runbook model, driven here as the
    // real two ops in the same order.
    let assert_out = run(
        &UboEdgeConnect,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": nominator, "to_entity_id": subject.0,
            "kind": "voting_rights", "pierced-from": nominee_edge,
        }),
        &pool,
    )
    .await;
    assert!(assert_out.get("seq").is_some(), "pierce's connect step must append: {assert_out:?}");
    let disconnect_out = run(
        &UboEdgeDisconnect,
        serde_json::json!({ "subject-id": subject.0, "edge-id": nominee_edge }),
        &pool,
    )
    .await;
    assert!(disconnect_out.get("seq").is_some(), "pierce's disconnect step must append: {disconnect_out:?}");

    let state = fold_subject(&pool, subject).await;
    let old = state
        .edges
        .get(&ob_poc_kyc_substrate::EdgeId(nominee_edge))
        .expect("nominee edge stays in the fold (K-13 supersede-never-delete)");
    assert_eq!(
        old.status,
        EdgeStatus::Superseded,
        "effect (a): the nominee edge must be Superseded"
    );
    assert!(
        old.superseded_by.is_some(),
        "effect (a): superseded_by must point at the supersede event (K-35)"
    );

    let new_edge = state
        .edges
        .values()
        .find(|e| e.from.0 == nominator && e.is_active())
        .expect("effect (b): a new active edge from the nominator must exist");
    assert_eq!(
        new_edge.kind,
        EdgeKind::VotingRights,
        "the new edge carries the UNDERLYING kind, never nominee"
    );
    assert_eq!(
        new_edge.to.0, subject.0,
        "the new edge targets the same controlled entity"
    );
    assert_eq!(
        new_edge.pierced_from,
        Some(ob_poc_kyc_substrate::EdgeId(nominee_edge)),
        "the new edge carries pierced_from provenance (§2.6)"
    );
    // TS.6 P2: the two effects now originate from TWO SEPARATE governed
    // events (connect's, then disconnect's) — the macro composition
    // trades the old single-event atomicity-of-record for scope-level
    // transactional atomicity (both commit or both roll back together);
    // neither event id equals the other's by construction.
    assert_ne!(
        new_edge.originating_event_id,
        old.superseded_by.unwrap(),
        "post-redesign, the connect and disconnect steps are distinct events"
    );

    cleanup(&pool, &[subject]).await;
}

// ── (b) pierce refuses a non-nominee target edge ────────────────────────────

#[tokio::test]
async fn b_pierce_refuses_non_nominee_target_edge() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let holder = Uuid::new_v4();
    let nominator = Uuid::new_v4();

    setup_subject(&pool, subject, &[nominator]).await;
    let connect_out = run(
        &UboEdgeConnect,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": holder, "to_entity_id": subject.0,
            "kind": "voting_rights",
        }),
        &pool,
    )
    .await;
    let edge = Uuid::parse_str(connect_out["edge_id"].as_str().expect("edge_id"))
        .expect("edge_id must be a valid UUID");

    // TS.6 P2 (K-G7): the "target is actually EdgeKind::Nominee" check moved
    // to `kyc_ubo.assert.edge.connect`'s op layer, gated on `pierced-from` —
    // exercised directly rather than via the retired standalone verb.
    let result = run_fallible(
        &UboEdgeConnect,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": nominator, "to_entity_id": subject.0,
            "kind": "voting_rights", "pierced-from": edge,
        }),
        &pool,
    )
    .await;
    assert!(
        result.is_err(),
        "connect's pierced-from check must refuse a target edge that is not \
         EdgeKind::Nominee"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("not a nominee edge"),
        "rejection must be attributable to the kind check; got: {msg}"
    );

    cleanup(&pool, &[subject]).await;
}

// ── (b2) normalizer refuses kind=nominee and unknown kinds ──────────────────

#[tokio::test]
async fn b2_pierce_normalizer_rejects_nominee_and_unknown_kinds() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let nominee = Uuid::new_v4();
    let nominator = Uuid::new_v4();

    setup_subject(&pool, subject, &[nominator]).await;
    let connect_out = run(
        &UboEdgeConnect,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": nominee, "to_entity_id": subject.0,
            "kind": "nominee",
        }),
        &pool,
    )
    .await;
    let edge = Uuid::parse_str(connect_out["edge_id"].as_str().expect("edge_id"))
        .expect("edge_id must be a valid UUID");

    // TS.6 P2 (K-G7): both checks moved to `kyc_ubo.assert.edge.connect`'s
    // normalizer, gated on `pierced-from` — exercised directly.
    //
    // A pierce cannot produce another nominee edge (fail-closed, §2.6).
    let nominee_kind = run_fallible(
        &UboEdgeConnect,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": nominator, "to_entity_id": subject.0,
            "kind": "nominee", "pierced-from": edge,
        }),
        &pool,
    )
    .await;
    assert!(nominee_kind.is_err(), "kind=nominee must be rejected");
    let msg = nominee_kind.unwrap_err().to_string();
    assert!(
        msg.contains("cannot produce another nominee edge"),
        "rejection must name the nominee-kind rule; got: {msg}"
    );

    // Unknown kind — the TS.1 §1b wire-normalizer discipline, fail-closed.
    let unknown = run_fallible(
        &UboEdgeConnect,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": nominator, "to_entity_id": subject.0,
            "kind": "votng_rights", "pierced-from": edge,
        }),
        &pool,
    )
    .await;
    assert!(
        unknown.is_err(),
        "an unknown kind must be rejected fail-closed"
    );
    let msg = unknown.unwrap_err().to_string();
    assert!(
        msg.contains("unrecognized kind"),
        "rejection must be the wire-normalizer's; got: {msg}"
    );

    cleanup(&pool, &[subject]).await;
}

// ── (c) pierce refuses inactive/superseded and non-existent targets ─────────

#[tokio::test]
async fn c_pierce_refuses_superseded_and_missing_targets() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let nominee = Uuid::new_v4();
    let nominator = Uuid::new_v4();

    setup_subject(&pool, subject, &[nominator]).await;
    let connect_out = run(
        &UboEdgeConnect,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": nominee, "to_entity_id": subject.0,
            "kind": "nominee",
        }),
        &pool,
    )
    .await;
    let edge = Uuid::parse_str(connect_out["edge_id"].as_str().expect("edge_id"))
        .expect("edge_id must be a valid UUID");
    run(
        &UboEdgeDisconnect,
        serde_json::json!({ "subject-id": subject.0, "edge-id": edge }),
        &pool,
    )
    .await;

    // TS.6 P2 (K-G7): both checks moved to `kyc_ubo.assert.edge.connect`'s
    // pierced-from pre-fold check — exercised directly.
    //
    // Disconnected target — the EdgeActive stud (matrix rows 3/4) refuses.
    let inactive = run_fallible(
        &UboEdgeConnect,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": nominator, "to_entity_id": subject.0,
            "kind": "voting_rights", "pierced-from": edge,
        }),
        &pool,
    )
    .await;
    assert!(
        inactive.is_err(),
        "pierce must refuse a disconnected (inactive) target edge"
    );

    // Non-existent target — refused before anything appends.
    let missing = run_fallible(
        &UboEdgeConnect,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": nominator, "to_entity_id": subject.0,
            "kind": "voting_rights", "pierced-from": Uuid::new_v4(),
        }),
        &pool,
    )
    .await;
    assert!(
        missing.is_err(),
        "pierce must refuse a non-existent target edge"
    );

    cleanup(&pool, &[subject]).await;
}

// ── (d) freeze fail-closes while unpierced nominee edges remain ─────────────
//
// §2.6: `resolve()` cannot error (trait signature), so the fail-closed scan
// lives at the freeze dispatch site.

#[tokio::test]
async fn d_freeze_hard_errors_while_unpierced_nominee_edge_active() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let nominee = Uuid::new_v4();
    let nominator = Uuid::new_v4();

    setup_subject(&pool, subject, &[nominator]).await;
    let connect_out = run(
        &UboEdgeConnect,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": nominee, "to_entity_id": subject.0,
            "kind": "nominee",
        }),
        &pool,
    )
    .await;
    let nominee_edge = Uuid::parse_str(connect_out["edge_id"].as_str().expect("edge_id"))
        .expect("edge_id must be a valid UUID");

    // T4-close: `EntityTypeSupportsStrategy` admits `llc_us`
    // (`control_prong_strategy`, §2) straight through — the unpierced-nominee
    // guard is UNCONDITIONAL and runs before strategy dispatch regardless
    // (TS.4 §3 Ruling B), so it is the ONLY refusal reachable here.
    let result = run_fallible(
        &UboDeterminationFreeze,
        serde_json::json!({ "subject-id": subject.0, "policy-version": "v1.0" }),
        &pool,
    )
    .await;
    assert!(
        result.is_err(),
        "freeze must hard-error while an unpierced nominee edge remains \
         active (K-8 fail-closed), regardless of which strategy the \
         subject's type would otherwise dispatch to"
    );
    let msg = result.unwrap_err().to_string();
    assert!(
        msg.contains("unpierced") && msg.contains(&nominee_edge.to_string()),
        "the error must name the unpierced edge(s); got: {msg}"
    );

    cleanup(&pool, &[subject]).await;
}

// ── (e) post-pierce the strategy delegates and resolves the nominator ───────

#[tokio::test]
async fn e_post_pierce_strategy_resolves_nominator_chain() {
    let pool = pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let nominee = Uuid::new_v4();
    let nominator = Uuid::new_v4();

    setup_subject(&pool, subject, &[nominator]).await;
    let connect_out = run(
        &UboEdgeConnect,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": nominee, "to_entity_id": subject.0,
            "kind": "nominee",
        }),
        &pool,
    )
    .await;
    let nominee_edge = Uuid::parse_str(connect_out["edge_id"].as_str().expect("edge_id"))
        .expect("edge_id must be a valid UUID");
    // TS.6 P2 (K-G7): pierce via the two real composed ops (macro-equivalent).
    run(
        &UboEdgeConnect,
        serde_json::json!({
            "subject-id": subject.0, "from_entity_id": nominator, "to_entity_id": subject.0,
            "kind": "voting_rights", "pierced-from": nominee_edge,
        }),
        &pool,
    )
    .await;
    run(
        &UboEdgeDisconnect,
        serde_json::json!({ "subject-id": subject.0, "edge-id": nominee_edge }),
        &pool,
    )
    .await;
    // EOP-DD-UBO-DISPATCH-001 T4-close (2026-08-28): no `EntityType`
    // dispatches to `nominee_pierce_strategy` any longer (§2 — "nowhere,
    // deliberately"). Post-pierce, `freeze` runs the K-8 unpierced-nominee
    // guard (finds nothing, the edge above disconnected it) and then
    // dispatches by the subject's real type (`llc_us` → §2
    // `control_prong_strategy`) — the SAME delegation
    // `NomineePierceStrategy` used to perform explicitly ("a thin delegate
    // to the control-prong traversal") now happens implicitly, because the
    // subject's own type was the real determinant all along.
    let candidates = freeze_candidates(&pool, subject, "control_prong_strategy").await;

    assert_eq!(
        candidates.len(),
        1,
        "expected exactly the disclosed nominator; got {candidates:#?}"
    );
    assert_eq!(
        candidates[0].get("person_id").and_then(|v| v.as_str()),
        Some(nominator.to_string().as_str()),
        "post-pierce, the NOMINATOR (never the nominee) is the candidate (K-8)"
    );
    assert_eq!(
        candidates[0].get("prong").and_then(|v| v.as_str()),
        Some("ControlByOtherMeans"),
        "K-1 basis: the delegated control walk yields ControlByOtherMeans"
    );
    assert!(
        candidates[0]
            .get("effective_ownership_pct")
            .map(|v| v.is_null())
            .unwrap_or(false),
        "control carries no quantum — effective_ownership_pct must be null"
    );

    cleanup(&pool, &[subject]).await;
}

// ── (f) freeze unknown-strategy arm — RETIRED (TS.6 P2) ─────────────────────
//
// `f_freeze_rejects_unknown_strategy_listing_all_eight` drove
// `select-strategy` with a `"no_such_strategy"` string to prove freeze's
// catch-all arm. That entry point is gone: the strategy is now DERIVED
// from `structure_class`, a TOTAL function over all 11 `StructureClass`
// variants (pinned by
// `kyc_pack_closure.rs::precondition_and_strategy_coverage_is_exactly_known`),
// so there is no governed way left to hand freeze an arbitrary strategy
// name — see the identical note on `kyc_ts1_trust.rs`'s deleted
// `f_freeze_still_rejects_unknown_strategy_after_trust_widening`.
