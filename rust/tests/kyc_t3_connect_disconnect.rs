//! T3 gate tests — `EOP-VS-UBO-GAME-001` §3.1/§3.2/§3.3/§3.4 (R8)/§8 Q3.
//!
//! P0d RED gates, written before `connect`/`disconnect` exist. `place`/
//! `remove` already exist (T2) and are used here for setup only, unchanged.
//! At RED time, every scenario that reaches a `connect`/`disconnect` call
//! fails there (`canonical_event_shape` bails on the unrecognized FQN) — the
//! earliest failure point available, same discipline as
//! `tests/kyc_t2_place_remove.rs`.
//!
//! R8 was CORRECTED mid-tranche (2026-08-27): the original ruling this file
//! shipped with — `remove` REFUSES while an active link touches the block —
//! was superseded before landing. The ratified reading is: `remove` is
//! NEVER refused for this reason. It PRUNES the links that touch the
//! removed block, computed at fold time from prior state (never in the
//! event payload — R6 purity), and nothing propagates past them. A block
//! left with zero links is an ordinary, legal board member, not an error
//! state. `remove_prunes_only_touching_links` and
//! `a_block_with_no_links_is_a_legal_board_state` replace the old
//! `remove_refuses_while_links_touch_the_block` gate; `a_board_unwinds_completely`
//! is restated below to match.
//!
//! - `connect_carries_its_kind` — one verb (the former `control` AND
//!   `economic-interest`), kind as a declared argument, geometry validating
//!   the classified pipe (TS.5 R1) on both a legal and an illegal triple;
//!   the system mints the link id (§8 Q3) and a caller-supplied id is
//!   refused.
//! - `disconnect_takes_the_link_off_the_board` — supersession, renamed:
//!   after disconnect the edge is still present (K-13, nothing deleted) but
//!   inactive.
//! - `remove_prunes_only_touching_links` — R8's prune ruling: removing a
//!   connected block deactivates only the links that touch it (never
//!   refused, never cascades) — a link one hop further away, and the
//!   far-end block's placement and every other link it holds, are
//!   byte-identical afterward.
//! - `a_block_with_no_links_is_a_legal_board_state` — a placed block with
//!   zero links is legal, enumerable as an ordinary `remove` candidate, and
//!   raises nothing.
//! - `a_board_unwinds_completely` — R8's acceptance test, restated: every
//!   prior board shape is reachable again, not every move has a symmetric
//!   inverse. `remove` prunes its own touching links as a side effect (no
//!   disconnect-first choreography needed); reaching a torn-down shape
//!   again takes MORE moves than the teardown did — `place` then an
//!   explicit re-`connect`, since `remove` does not restore what it pruned.

use std::sync::Arc;

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use dsl_runtime::TransactionScope;
use ob_poc::domain_ops::kyc_workbook::open_workbook;
use ob_poc_kyc_seam::{append_in_scope, canonical_event_shape};
use ob_poc_kyc_store::PgKycEventStore;
use ob_poc_kyc_substrate::{
    assembly_lexicon, check_control_preconditions, enumerate_placement_set,
    fold_control_versioned, fold_type_registry, AuthorityRef, ControlState, EdgeId, EntityId,
    FoldRegistry, IntentEvent, LexiconManifest, ObligationState, Principal, SubjectId,
    TargetBinding, TypeRegistryState, V1FoldImpl,
};
use ob_poc_types::TransactionScopeId;
use sem_os_core::principal::Principal as RuntimePrincipal;

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string())
}

async fn connect_pool() -> PgPool {
    PgPoolOptions::new()
        .max_connections(8)
        .connect(&database_url())
        .await
        .expect("connect to test DB")
}

fn v1_registry() -> FoldRegistry {
    let mut r = FoldRegistry::new();
    r.register(assembly_lexicon().hash, Arc::new(V1FoldImpl));
    r
}

fn runtime_principal(actor_id: &str) -> RuntimePrincipal {
    RuntimePrincipal {
        actor_id: actor_id.to_string(),
        roles: vec!["analyst".to_string()],
        claims: std::collections::HashMap::new(),
        tenancy: None,
    }
}

fn fixed_ts() -> chrono::DateTime<chrono::Utc> {
    chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z")
        .unwrap()
        .with_timezone(&chrono::Utc)
}

struct TestScope {
    tx: Transaction<'static, Postgres>,
    pool: PgPool,
    id: TransactionScopeId,
}
impl TestScope {
    async fn begin(pool: &PgPool) -> Self {
        Self {
            tx: pool.begin().await.unwrap(),
            pool: pool.clone(),
            id: TransactionScopeId::new(),
        }
    }
    async fn commit(self) {
        self.tx.commit().await.unwrap();
    }
}
impl TransactionScope for TestScope {
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
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".kyc_intent_events WHERE subject_root = $1"#)
        .bind(subject.0)
        .execute(pool)
        .await;
    let _ = sqlx::query(r#"DELETE FROM "ob-poc".kyc_subject_streams WHERE subject_root = $1"#)
        .bind(subject.0)
        .execute(pool)
        .await;
}

/// Op-layer simulation: build via the ONE constructor (R6), append under the
/// real precondition oracle. Not the real `SemOsVerbOp` (those structs don't
/// exist yet for connect/disconnect at RED time) — `canonical_event_shape` +
/// `check_control_preconditions` are the two real pieces the op wraps; this
/// inlines that wrapping so the gate compiles and REDs from day one, same
/// reason `kyc_t2_place_remove.rs` never referenced `KycSubjectPlace`.
async fn apply(
    scope: &mut TestScope,
    registry: &FoldRegistry,
    lexicon: &LexiconManifest,
    subject: SubjectId,
    fqn: &str,
    args: &serde_json::Value,
    as_of: chrono::DateTime<chrono::Utc>,
) -> anyhow::Result<Option<EdgeId>> {
    let (target, payload, edge) = canonical_event_shape(fqn, subject, args)?;
    let entry = lexicon.get(fqn).cloned();
    let event = IntentEvent::new(
        subject,
        fqn,
        Principal::test_analyst(),
        AuthorityRef("test".into()),
        target,
        payload,
        as_of,
    )
    .with_lexicon_hash(lexicon.hash);
    append_in_scope(scope, registry, &event, "test", |control, _obligation, type_registry| {
        if let Some(e) = &entry {
            check_control_preconditions(e, control, type_registry, &event)
        } else {
            Ok(())
        }
    })
    .await
    .map_err(|e| anyhow::anyhow!("{fqn} append failed: {e}"))?;
    Ok(edge)
}

async fn reload(pool: &PgPool, subject: SubjectId, registry: &FoldRegistry) -> (ControlState, TypeRegistryState) {
    let mut conn = pool.acquire().await.unwrap();
    let events = PgKycEventStore::load_events(&mut conn, subject).await.unwrap();
    let refs: Vec<&IntentEvent> = events.iter().collect();
    let control = fold_control_versioned(&refs, registry).expect("fold ok");
    let type_registry = fold_type_registry(&refs);
    (control, type_registry)
}

// ── connect_carries_its_kind ────────────────────────────────────────────────

#[tokio::test]
async fn connect_carries_its_kind() {
    let pool = connect_pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let lexicon = assembly_lexicon();
    let registry = v1_registry();
    let as_of = fixed_ts();

    let to = EntityId(subject.0);
    let from = EntityId(Uuid::new_v4());
    let to2 = EntityId(Uuid::new_v4());

    let mut scope = TestScope::begin(&pool).await;
    apply(&mut scope, &registry, &lexicon, subject, "kyc_ubo.assert.subject.place",
        &serde_json::json!({ "entity-id": to.0, "entity-type": "private_limited_company" }), as_of)
        .await.expect("place (T2, already real) must succeed");
    apply(&mut scope, &registry, &lexicon, subject, "kyc_ubo.assert.subject.place",
        &serde_json::json!({ "entity-id": from.0, "entity-type": "natural_person" }), as_of)
        .await.expect("place (T2, already real) must succeed");
    apply(&mut scope, &registry, &lexicon, subject, "kyc_ubo.assert.subject.place",
        &serde_json::json!({ "entity-id": to2.0, "entity-type": "private_limited_company" }), as_of)
        .await.expect("place (T2, already real) must succeed");

    // §3.2: connect absorbs assert-control — kind carries the classification.
    let connect_result = apply(
        &mut scope, &registry, &lexicon, subject, "kyc_ubo.assert.edge.connect",
        &serde_json::json!({
            "from_entity_id": from.0, "to_entity_id": to.0, "kind": "voting_rights",
        }),
        as_of,
    ).await;
    let edge1 = match connect_result {
        Err(e) => panic!(
            "RED (expected pre-P1): kyc_ubo.assert.edge.connect not yet buildable — {e:?}"
        ),
        Ok(edge) => edge.expect("§8 Q3: connect always mints or accepts an edge id"),
    };
    scope.commit().await;

    let (control, _tr) = reload(&pool, subject, &registry).await;
    let e1 = control.edges.get(&edge1).expect("edge1 must be in the folded state");
    assert_eq!(e1.from, from);
    assert_eq!(e1.to, to);
    assert!(e1.is_active());
    assert!(
        matches!(e1.kind, ob_poc_kyc_substrate::EdgeKind::VotingRights),
        "connect must carry the declared kind through to the fold, got {:?}", e1.kind
    );

    // §3.2: connect ALSO absorbs assert-economic-interest — same verb, kind
    // "economic_interest", percentage attribute. Distinct kind from edge1 on
    // the SAME (from,to) pair, so NoDuplicateActiveEdge admits both.
    let mut scope2 = TestScope::begin(&pool).await;
    let mut conn = pool.acquire().await.unwrap();
    let mut workbook = open_workbook(&mut conn, subject).await.unwrap();
    drop(conn);
    let principal = runtime_principal("analyst-1");
    workbook
        .stage(
            &format!(
                r#"(kyc_ubo.assert.edge.connect :subject-id "{subject}" :from_entity_id "{from}" :to_entity_id "{to}" :kind "economic_interest" :percentage 40)"#,
                subject = subject.0, from = from.0, to = to.0
            ),
            &principal,
            AuthorityRef("analyst.connect".into()),
            as_of,
        )
        .expect("workbook-layer must also recognise connect (both surfaces, R6)");
    workbook.commit(&mut scope2, &registry).await.expect("workbook commit must succeed");
    scope2.commit().await;

    let (control2, _tr2) = reload(&pool, subject, &registry).await;
    let economic_edge = control2
        .edges
        .values()
        .find(|e| e.from == from && e.to == to && matches!(e.kind, ob_poc_kyc_substrate::EdgeKind::EconomicInterest))
        .expect("the economic-interest-kind connect must have landed as a distinct edge");
    assert_eq!(economic_edge.percentage, Some(40.0), "percentage must carry through");

    // TS.5 R1: geometry validates the CLASSIFIED pipe, not the raw kind —
    // OfficerAppointment is natural-person-source-only (source_permits);
    // `to` (a corporate) as source must be refused.
    let mut scope3 = TestScope::begin(&pool).await;
    let illegal = apply(
        &mut scope3, &registry, &lexicon, subject, "kyc_ubo.assert.edge.connect",
        &serde_json::json!({
            "from_entity_id": to.0, "to_entity_id": to2.0, "kind": "officer_appointment",
        }),
        as_of,
    ).await;
    scope3.commit().await;
    let err = illegal.expect_err("geometry must refuse a corporate source for officer_appointment");
    let msg = err.to_string();
    assert!(
        msg.contains("geometry") || msg.contains("not a move"),
        "expected a geometry-shaped refusal, got: {msg}"
    );

    // §8 Q3: a caller-supplied edge id is refused — the system mints it.
    let mut scope4 = TestScope::begin(&pool).await;
    let caller_supplied = apply(
        &mut scope4, &registry, &lexicon, subject, "kyc_ubo.assert.edge.connect",
        &serde_json::json!({
            "from_entity_id": from.0, "to_entity_id": to2.0, "kind": "voting_rights",
            "edge-id": Uuid::new_v4().to_string(),
        }),
        as_of,
    ).await;
    scope4.commit().await;
    caller_supplied.expect_err(
        "§8 Q3: a caller-chosen edge id is a stored identifier the board cannot predict — refused",
    );

    cleanup(&pool, subject).await;
}

// ── disconnect_takes_the_link_off_the_board ─────────────────────────────────

#[tokio::test]
async fn disconnect_takes_the_link_off_the_board() {
    let pool = connect_pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let lexicon = assembly_lexicon();
    let registry = v1_registry();
    let as_of = fixed_ts();

    let to = EntityId(subject.0);
    let from = EntityId(Uuid::new_v4());

    let mut scope = TestScope::begin(&pool).await;
    apply(&mut scope, &registry, &lexicon, subject, "kyc_ubo.assert.subject.place",
        &serde_json::json!({ "entity-id": to.0, "entity-type": "private_limited_company" }), as_of)
        .await.expect("place (T2, already real) must succeed");
    apply(&mut scope, &registry, &lexicon, subject, "kyc_ubo.assert.subject.place",
        &serde_json::json!({ "entity-id": from.0, "entity-type": "natural_person" }), as_of)
        .await.expect("place (T2, already real) must succeed");

    let edge = match apply(
        &mut scope, &registry, &lexicon, subject, "kyc_ubo.assert.edge.connect",
        &serde_json::json!({ "from_entity_id": from.0, "to_entity_id": to.0, "kind": "voting_rights" }),
        as_of,
    ).await {
        Err(e) => panic!("RED (expected pre-P1): kyc_ubo.assert.edge.connect not yet buildable — {e:?}"),
        Ok(edge) => edge.expect("connect always mints or accepts an edge id"),
    };
    scope.commit().await;

    let mut scope2 = TestScope::begin(&pool).await;
    match apply(
        &mut scope2, &registry, &lexicon, subject, "kyc_ubo.assert.edge.disconnect",
        &serde_json::json!({ "edge-id": edge.0.to_string() }),
        as_of,
    ).await {
        Err(e) => panic!("RED (expected pre-P2): kyc_ubo.assert.edge.disconnect not yet buildable — {e:?}"),
        Ok(_) => {}
    }
    scope2.commit().await;

    let (control, _tr) = reload(&pool, subject, &registry).await;
    let e = control.edges.get(&edge).expect("K-13: disconnect must not delete the edge");
    assert!(!e.is_active(), "disconnect must take the link off the board (inactive)");

    // Both surfaces: a second link, disconnected through the workbook.
    let mut scope3 = TestScope::begin(&pool).await;
    let edge2 = apply(
        &mut scope3, &registry, &lexicon, subject, "kyc_ubo.assert.edge.connect",
        &serde_json::json!({ "from_entity_id": from.0, "to_entity_id": to.0, "kind": "board_appointment" }),
        as_of,
    ).await.expect("second connect (different kind, no duplicate conflict)")
        .expect("connect always mints or accepts an edge id");
    scope3.commit().await;

    let mut scope4 = TestScope::begin(&pool).await;
    let mut conn = pool.acquire().await.unwrap();
    let mut workbook = open_workbook(&mut conn, subject).await.unwrap();
    drop(conn);
    let principal = runtime_principal("analyst-1");
    workbook
        .stage(
            &format!(r#"(kyc_ubo.assert.edge.disconnect :subject-id "{subject}" :edge-id "{edge2}")"#,
                subject = subject.0, edge2 = edge2.0),
            &principal,
            AuthorityRef("analyst.disconnect".into()),
            as_of,
        )
        .expect("workbook-layer must also recognise disconnect (both surfaces, R6)");
    workbook.commit(&mut scope4, &registry).await.expect("workbook commit must succeed");
    scope4.commit().await;

    let (control2, _tr2) = reload(&pool, subject, &registry).await;
    assert!(
        !control2.edges.get(&edge2).unwrap().is_active(),
        "workbook-layer disconnect must also take the link off the board"
    );

    cleanup(&pool, subject).await;
}

// ── remove_prunes_only_touching_links ───────────────────────────────────────

#[tokio::test]
async fn remove_prunes_only_touching_links() {
    let pool = connect_pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let lexicon = assembly_lexicon();
    let registry = v1_registry();
    let as_of = fixed_ts();

    let a = EntityId(subject.0); // the block we remove
    let b = EntityId(Uuid::new_v4()); // touches A directly
    let c = EntityId(Uuid::new_v4()); // touches B, NOT A — one hop further

    let mut scope = TestScope::begin(&pool).await;
    for (id, wire) in [
        (a, "private_limited_company"),
        (b, "natural_person"),
        (c, "private_limited_company"),
    ] {
        apply(&mut scope, &registry, &lexicon, subject, "kyc_ubo.assert.subject.place",
            &serde_json::json!({ "entity-id": id.0, "entity-type": wire }), as_of)
            .await.expect("place (T2, already real) must succeed");
    }
    let edge_ba = apply(
        &mut scope, &registry, &lexicon, subject, "kyc_ubo.assert.edge.connect",
        &serde_json::json!({ "from_entity_id": b.0, "to_entity_id": a.0, "kind": "voting_rights" }),
        as_of,
    ).await.expect("connect must build (P1 landed)").expect("connect mints an id");
    // B (natural person) is also an officer of C (a company) — a link that
    // touches B, but NOT A. Proves the pruning below does not cascade past
    // A's own touching links.
    let edge_bc = apply(
        &mut scope, &registry, &lexicon, subject, "kyc_ubo.assert.edge.connect",
        &serde_json::json!({ "from_entity_id": b.0, "to_entity_id": c.0, "kind": "officer_appointment" }),
        as_of,
    ).await.expect("connect must build (P1 landed)").expect("connect mints an id");
    scope.commit().await;

    // R8 (corrected): remove(A) is NEVER refused, even though an active
    // link (edge_ba) touches it — it prunes that link as a fold-time side
    // effect instead.
    let mut scope2 = TestScope::begin(&pool).await;
    apply(&mut scope2, &registry, &lexicon, subject, "kyc_ubo.assert.subject.remove",
        &serde_json::json!({ "entity-id": a.0 }), as_of)
        .await.expect("R8 (corrected): remove must never refuse for a touching link");
    scope2.commit().await;

    let (control, tr) = reload(&pool, subject, &registry).await;

    // PRUNE: edge_ba (touches A) is now inactive, and traceable to the
    // remove event that pruned it (K-35).
    let e_ba = control.edges.get(&edge_ba).expect("edge_ba must still be present (K-13)");
    assert!(!e_ba.is_active(), "R8: the link touching the removed block must be pruned");
    assert!(e_ba.superseded_by.is_some(), "K-35: the pruning event must be traceable");

    // DO NOT CASCADE: edge_bc is one hop further away (touches B and C, not
    // A) and must be completely untouched.
    let e_bc = control.edges.get(&edge_bc).expect("edge_bc must still be present");
    assert!(e_bc.is_active(), "R8: nothing may propagate past the touching link — edge_bc must stay active");
    assert_eq!(e_bc.from, b);
    assert_eq!(e_bc.to, c);

    // The far-end blocks are byte-identical afterward: same placement, same
    // every other link. B and C are unaffected by A's removal.
    assert!(!tr.is_withdrawn(b), "R8: B's placement must be untouched by A's removal");
    assert!(!tr.is_withdrawn(c), "R8: C's placement must be untouched by A's removal");
    assert!(control.registered_entity_ids.contains(&b));
    assert!(control.registered_entity_ids.contains(&c));

    // A itself: withdrawn, never deleted (§2).
    assert!(tr.is_withdrawn(a));
    assert!(control.registered_entity_ids.contains(&a));

    cleanup(&pool, subject).await;
}

// ── a_block_with_no_links_is_a_legal_board_state ────────────────────────────

#[tokio::test]
async fn a_block_with_no_links_is_a_legal_board_state() {
    let pool = connect_pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let lexicon = assembly_lexicon();
    let registry = v1_registry();
    let as_of = fixed_ts();

    let d = EntityId(Uuid::new_v4());

    let mut scope = TestScope::begin(&pool).await;
    apply(&mut scope, &registry, &lexicon, subject, "kyc_ubo.assert.subject.place",
        &serde_json::json!({ "entity-id": d.0, "entity-type": "natural_person" }), as_of)
        .await.expect("place (T2, already real) must succeed");
    scope.commit().await;

    let (control, tr) = reload(&pool, subject, &registry).await;
    assert!(control.registered_entity_ids.contains(&d));
    assert!(
        !tr.is_withdrawn(d),
        "a freshly placed block with zero links is a normal, non-withdrawn board member"
    );
    assert!(
        control.edges.values().all(|e| e.from != d && e.to != d),
        "D has zero links by construction"
    );

    // ENUMERABLE: a placed, zero-link block is an ordinary `remove`
    // candidate — no special-casing excludes it.
    let obligation = ObligationState::default();
    let placement_set = enumerate_placement_set(subject, &control, &obligation, &tr, &lexicon);
    let target = TargetBinding {
        entity_id: Some(d),
        ..TargetBinding::for_subject(subject)
    };
    assert!(
        placement_set.admits("kyc_ubo.assert.subject.remove", &target),
        "a zero-link block must enumerate as a legal remove candidate"
    );

    // RAISES NOTHING: remove(D) succeeds outright — no links to prune, no
    // special case, no error.
    let mut scope2 = TestScope::begin(&pool).await;
    apply(&mut scope2, &registry, &lexicon, subject, "kyc_ubo.assert.subject.remove",
        &serde_json::json!({ "entity-id": d.0 }), as_of)
        .await.expect("removing a zero-link block must raise nothing");
    scope2.commit().await;

    cleanup(&pool, subject).await;
}

// ── a_board_unwinds_completely (R8 acceptance test, restated) ───────────────

#[tokio::test]
async fn a_board_unwinds_completely() {
    let pool = connect_pool().await;
    let subject = SubjectId(Uuid::new_v4());
    let lexicon = assembly_lexicon();
    let registry = v1_registry();
    let as_of = fixed_ts();

    let a = EntityId(subject.0); // corporate hub
    let b = EntityId(Uuid::new_v4()); // natural person

    // Build: A, B placed, one active link B→A.
    let mut scope = TestScope::begin(&pool).await;
    for (id, wire) in [(a, "private_limited_company"), (b, "natural_person")] {
        apply(&mut scope, &registry, &lexicon, subject, "kyc_ubo.assert.subject.place",
            &serde_json::json!({ "entity-id": id.0, "entity-type": wire }), as_of)
            .await.expect("place (T2, already real) must succeed");
    }
    let edge1 = apply(
        &mut scope, &registry, &lexicon, subject, "kyc_ubo.assert.edge.connect",
        &serde_json::json!({ "from_entity_id": b.0, "to_entity_id": a.0, "kind": "voting_rights" }),
        as_of,
    ).await.expect("connect must build (P1 landed)").expect("connect mints an id");
    scope.commit().await;

    // Tear down: remove(A) directly — R8 (corrected) means no
    // disconnect-first choreography is needed. This is NEVER refused; it
    // prunes edge1 as a fold-time side effect.
    let mut s1 = TestScope::begin(&pool).await;
    apply(&mut s1, &registry, &lexicon, subject, "kyc_ubo.assert.subject.remove",
        &serde_json::json!({ "entity-id": a.0 }), as_of)
        .await.expect("R8 (corrected): remove must never refuse for a touching link");
    s1.commit().await;

    let (control, tr) = reload(&pool, subject, &registry).await;
    assert!(tr.is_withdrawn(a), "A is withdrawn after remove");
    assert!(!tr.is_withdrawn(b), "B is untouched by A's removal");
    assert!(
        !control.edges.get(&edge1).unwrap().is_active(),
        "edge1 was pruned as a side effect of removing A"
    );
    // A board with A withdrawn, B still placed, and edge1 inactive is a
    // perfectly ordinary intermediate state — nothing here is an error or
    // something to clean up.

    // Reach the PRIOR SHAPE again: R8 is "every prior board shape is
    // reachable again", not "every move has a symmetric inverse." `place`
    // alone does not restore edge1 — it stays pruned forever (K-13,
    // supersede-never-delete) — reaching the same shape needs an explicit
    // re-`connect`, i.e. MORE moves than the teardown took.
    let mut s2 = TestScope::begin(&pool).await;
    apply(&mut s2, &registry, &lexicon, subject, "kyc_ubo.assert.subject.place",
        &serde_json::json!({ "entity-id": a.0, "entity-type": "private_limited_company" }), as_of)
        .await.expect("A can be re-placed after removal (T2)");
    s2.commit().await;

    let (control, tr) = reload(&pool, subject, &registry).await;
    assert!(!tr.is_withdrawn(a), "A is placed again");
    assert!(
        !control.edges.get(&edge1).unwrap().is_active(),
        "R8: place alone does NOT restore a pruned link — edge1 stays inactive forever"
    );
    assert!(
        control.edges.values().all(|e| e.to != a || !e.is_active()),
        "no active link into A exists yet — reaching the prior shape isn't done by place alone"
    );

    let mut s3 = TestScope::begin(&pool).await;
    let edge2 = apply(
        &mut s3, &registry, &lexicon, subject, "kyc_ubo.assert.edge.connect",
        &serde_json::json!({ "from_entity_id": b.0, "to_entity_id": a.0, "kind": "voting_rights" }),
        as_of,
    ).await.expect("re-connect must build").expect("connect mints an id");
    s3.commit().await;

    let (control, tr) = reload(&pool, subject, &registry).await;
    assert!(!tr.is_withdrawn(a) && !tr.is_withdrawn(b), "the prior shape's placements are reachable again");
    let e2 = control.edges.get(&edge2).expect("edge2 must be present");
    assert!(e2.is_active(), "the prior shape's active link (B→A, voting_rights) is reachable again");
    assert_eq!(e2.from, b);
    assert_eq!(e2.to, a);
    assert_ne!(edge2, edge1, "reaching the prior shape minted a NEW link — edge1 itself is never restored");

    cleanup(&pool, subject).await;
}
