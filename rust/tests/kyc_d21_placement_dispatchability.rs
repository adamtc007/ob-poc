//! EOP-DD-KYCUBO-D2.1 reconciliation, 2026-08-24 corrective tranche Item 3b.
//!
//! `every_advertised_move_is_dispatchable` — the GENERAL form of the defect
//! Item 3 found: `enumerate_placement_set` proposed a "type-scoped
//! attach-evidence" move (`crates/ob-poc-kyc-substrate/src/placement.rs`,
//! `type_registry_candidates`) for any entity with an asserted,
//! non-withdrawn type, but the sole registered op for that FQN
//! (`kyc_ubo.assert.edge.evidence`) hardcodes edge-scoping and rejects a
//! call shaped like the move it advertised (`Missing edge-id argument`).
//! This is the preview-vs-reality divergence TS.5 R2 closed for geometry;
//! this gate is the same discipline applied to dispatch shape, and it
//! should outlive this one instance.
//!
//! Method: build a real, populated board (pure, in-memory — no DB needed to
//! construct it), enumerate its real placement set, and for every DISTINCT
//! (verb_fqn, target-shape) pair the board proposes, dispatch the REAL
//! registered `SemOsVerbOp` (via `sem_os_postgres::ops::build_registry()` +
//! `ob_poc::domain_ops::extend_registry()` — the same registry
//! `ob-poc-web::main` builds for production) with args matching ONLY that
//! shape, inside a transaction that is ALWAYS rolled back. A shape mismatch
//! surfaces as `Missing entity-id argument` / `Missing edge-id argument`
//! specifically for a field the move's own target did not supply — any
//! OTHER error (a legitimate business-logic refusal against a throwaway
//! board) is not this test's concern and is accepted.
//!
//! **Item 3a closed (2026-08-24, same corrective tranche):** ruled to wire
//! the unscoped dispatch path (`UboEdgeAttachEvidence` now accepts
//! `entity-id` as an alternative to `edge-id`; the lexicon's
//! `EdgeExists`/`EdgeActive` preconditions became vacuous when `edge_id` is
//! absent, mirroring `PriorTypeAsserted`/`MembershipActive`'s existing
//! vacuous-when-probed-without-`entity_id` convention). The pin below
//! SHRANK TO EMPTY as the closing proof — re-ran this exact gate before and
//! after the fix; it genuinely flipped from red to green, not edited to
//! match. The pin idiom (matching `kyc_pack_closure.rs`'s several "X is
//! exactly known" gates) stays, empty, as a live regression guard: any
//! FUTURE preview-vs-reality divergence of this shape goes red immediately
//! rather than silently shipping.

use std::collections::BTreeSet;

use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

use chrono::{DateTime, Utc};
use dsl_runtime::{TransactionScope, VerbExecutionContext};
use ob_poc_kyc_substrate::{
    assembly_lexicon, enumerate_placement_set, fold_control, fold_type_registry, AuthorityRef,
    EntityId, IntentEvent, Principal, SubjectId, TargetBinding,
};
use ob_poc_types::TransactionScopeId;

fn database_url() -> String {
    std::env::var("DATABASE_URL").unwrap_or_else(|_| "postgresql:///data_designer".to_string())
}

async fn pool() -> PgPool {
    PgPoolOptions::new().max_connections(4).connect(&database_url()).await.expect("connect to test DB")
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

fn as_of() -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(0, 0).unwrap()
}

/// A real, populated board — one company, one natural person, both
/// registered and type-asserted (Alleged), a control edge between them.
/// Pure: built from `IntentEvent`s and folded directly, no DB round-trip.
fn populated_board(subject: SubjectId) -> (ob_poc_kyc_substrate::ControlState, ob_poc_kyc_substrate::TypeRegistryState) {
    let co = EntityId(Uuid::new_v4());
    let p1 = EntityId(Uuid::new_v4());
    let mk = |fqn: &str, target: TargetBinding, payload: serde_json::Value| {
        IntentEvent::new(subject, fqn, Principal::test_analyst(), AuthorityRef("test".into()), target, payload, as_of())
    };
    // Was `register` + `subject.type` (two events per entity) — retired
    // (EOP-DD-UBO-CLEANOUT-001 T6 P2, 2026-09-07): `place` absorbs both
    // axes in one event (T4/EOP-VS-UBO-GAME-001 §3.2).
    let events = [
        mk(
            "kyc_ubo.assert.subject.place",
            TargetBinding { entity_id: Some(co), ..TargetBinding::for_subject(subject) },
            serde_json::json!({ "entity_id": co.0, "entity_type": "private_limited_company" }),
        ),
        mk(
            "kyc_ubo.assert.subject.place",
            TargetBinding { entity_id: Some(p1), ..TargetBinding::for_subject(subject) },
            serde_json::json!({ "entity_id": p1.0, "entity_type": "natural_person" }),
        ),
    ];
    let refs: Vec<&IntentEvent> = events.iter().collect();
    let control = fold_control(&refs);
    let type_registry = fold_type_registry(&refs);
    (control, type_registry)
}

/// The three target shapes a `LegalMove` can declare (mirrors
/// `TargetBinding`'s discriminating fields for dispatch purposes).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Shape {
    SubjectOnly,
    EntityOnly,
    Edge,
}

fn shape_of(target: &TargetBinding) -> Shape {
    if target.edge_id.is_some() {
        Shape::Edge
    } else if target.entity_id.is_some() {
        Shape::EntityOnly
    } else {
        Shape::SubjectOnly
    }
}

fn args_for_shape(subject: Uuid, shape: Shape) -> serde_json::Value {
    let mut m = serde_json::json!({ "subject-id": subject });
    match shape {
        Shape::SubjectOnly => {}
        Shape::EntityOnly => {
            m["entity-id"] = serde_json::json!(Uuid::new_v4());
        }
        Shape::Edge => {
            m["edge-id"] = serde_json::json!(Uuid::new_v4());
        }
    }
    m
}

#[tokio::test]
async fn every_advertised_move_is_dispatchable() {
    let pool = pool().await;

    let subject = SubjectId(Uuid::new_v4());
    let (control, type_registry) = populated_board(subject);
    let placement = enumerate_placement_set(subject, &control, &type_registry, &assembly_lexicon());

    // Distinct (verb_fqn, shape) pairs the board actually proposes.
    let mut pairs: BTreeSet<(String, Shape)> = BTreeSet::new();
    for mv in &placement.moves {
        if mv.verb_fqn.0 == ob_poc_kyc_substrate::NONE_OF_THE_ABOVE {
            continue; // the synthetic abstention move, not a real dispatch target
        }
        pairs.insert((mv.verb_fqn.0.clone(), shape_of(&mv.target)));
    }
    // EOP-DD-UBO-DISPATCH-001 T4 (2026-08-28): the floor was `> 5` (implying
    // >=6) while `structure-class` was a live proposable move for a
    // registered/typed entity; its retirement dropped exactly one distinct
    // (verb_fqn, shape) pair from this board (6->5) — a real, expected
    // consequence of the move no longer existing, not a dispatch-shape
    // regression this gate exists to catch. Floor lowered by one to match.
    assert!(pairs.len() > 4, "sanity: the populated board must propose a non-trivial move set, got {}", pairs.len());

    let mut registry = sem_os_postgres::ops::build_registry();
    ob_poc::domain_ops::extend_registry(&mut registry);

    let mut undispatchable: BTreeSet<(String, String)> = BTreeSet::new();
    for (fqn, shape) in &pairs {
        let Some(op) = registry.get(fqn) else {
            // Missing registration entirely is a DIFFERENT, already-covered
            // defect (`every_declared_verb_has_a_registered_op`,
            // `tests/kyc_pack_closure.rs`) — not this gate's concern.
            continue;
        };
        let args = args_for_shape(subject.0, *shape);
        let mut ctx = VerbExecutionContext::default();
        let mut scope = Scope::begin(&pool).await;
        let result = op.execute(&args, &mut ctx, &mut scope).await;
        let _ = scope.tx.rollback().await;

        if let Err(e) = result {
            let msg = e.to_string();
            // The defect: the op demands a field THIS SHAPE DOES NOT SUPPLY
            // — not a field it does. EntityOnly supplies entity-id, not
            // edge-id; Edge supplies edge-id, not entity-id; SubjectOnly
            // supplies neither.
            let shape_field_missing = match shape {
                Shape::EntityOnly => msg.contains("Missing edge-id argument"),
                Shape::Edge => msg.contains("Missing entity-id argument"),
                Shape::SubjectOnly => msg.contains("Missing entity-id argument") || msg.contains("Missing edge-id argument"),
            };
            if shape_field_missing {
                undispatchable.insert((fqn.clone(), format!("{shape:?}")));
            }
        }
    }

    // Empty since Item 3a landed (2026-08-24) — see this file's header.
    // Live regression guard: any FUTURE divergence goes red here immediately.
    let known_undispatchable: BTreeSet<(String, String)> = BTreeSet::new();

    assert_eq!(
        undispatchable, known_undispatchable,
        "the set of (verb, shape) pairs the board advertises as legal but the registered op \
         rejects on shape alone has changed. If this GREW, a new preview-vs-reality divergence \
         was introduced — fix it, don't widen the pin. If this SHRANK to empty, Item 3a's fix \
         landed — narrow the pin to {{}} and remove this comment."
    );
}
