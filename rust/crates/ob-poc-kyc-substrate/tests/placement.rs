//! T2 gate tests — EOP-PLAN-KYCUBO-KIT-001 §T2 (closes KIT-3).
//!
//! `enumerate_placement_set` is pure: `(subject, state, lexicon) ->
//! PlacementSet`. These tests prove the four T2 gates: legality tracks the
//! same precondition oracle the write path uses, the set is deterministic,
//! ordering is canonical, and abstention is always present.

use std::time::Instant;

use uuid::Uuid;

use std::collections::BTreeMap;

use ob_poc_kyc_substrate::{
    check_preconditions, enumerate_placement_set, assembly_lexicon, ControlState, EdgeId,
    EdgeKind, EdgeState, EdgeStatus, EntityId, EventId, LexiconManifest,
    PlacementSet, ProofKind, ProofRecord, StructureClass, SubjectId, TargetBinding,
    TypeRegistryState,
};

fn subject() -> SubjectId {
    SubjectId(Uuid::new_v4())
}

fn empty_state() -> ControlState {
    ControlState::default()
}

fn empty_type_registry() -> TypeRegistryState {
    TypeRegistryState::default()
}

fn edge(status: EdgeStatus) -> EdgeState {
    EdgeState {
        id: EdgeId(Uuid::new_v4()),
        kind: EdgeKind::VotingRights,
        from: EntityId(Uuid::new_v4()),
        to: EntityId(Uuid::new_v4()),
        percentage: None,
        status,
        proofs: BTreeMap::new(),
        originating_event_id: EventId::new(),
        trust_revocable: None,
        superseded_by: None,
        pierced_from: None,
    }
}

/// EOP-DD-UBO-PROOF-001 §4 (T5): the only surviving axis `EdgeStatus`
/// itself dropped — an `Asserted` edge with a proof logged against it.
fn edge_with_proof() -> EdgeState {
    let mut e = edge(EdgeStatus::Asserted);
    let citing = EventId::new();
    e.proofs.insert(
        citing,
        ProofRecord {
            kind: ProofKind::FiledDocument,
            source: "test fixture".to_string(),
            date: "2026-08-28".to_string(),
            event_id: citing,
        },
    );
    e
}

fn state_with_edge(status: EdgeStatus) -> ControlState {
    let mut state = ControlState::default();
    let e = edge(status);
    state.edges.insert(e.id, e);
    state
}

fn state_with_edge_with_proof() -> ControlState {
    let mut state = ControlState::default();
    let e = edge_with_proof();
    state.edges.insert(e.id, e);
    state
}

fn strategized_state() -> ControlState {
    // TS.6 P2: strategy is derived from `structure_class`
    // (`select-strategy` retired) — `PrivateCompany` maps to
    // `ownership_prong_strategy` (`strategy_for_structure_class`).
    // EOP-VS-UBO-GAME-001 T3: no `reconciliation_event_id` field any more
    // (§3.3 — the K-14 gate it fed is dissolved with reconciliation itself).
    ControlState {
        structure_class: Some(StructureClass::PrivateCompany),
        classify_event_id: Some(EventId::new()),
        ..Default::default()
    }
}

// ── abstain_always_present ─────────────────────────────────────────────────

#[test]
fn abstain_always_present() {
    let subj = subject();
    let lexicon = assembly_lexicon();

    for state in [
        empty_state(),
        state_with_edge(EdgeStatus::Asserted),
        state_with_edge_with_proof(),
        strategized_state(),
    ] {
        let set = enumerate_placement_set(subj, &state, &empty_type_registry(), &lexicon);
        assert!(
            set.moves
                .iter()
                .any(|m| m.move_id == PlacementSet::abstain_move_id()),
            "abstention move must be present regardless of state"
        );
    }
}

// ── placement_set_deterministic ────────────────────────────────────────────

#[test]
fn placement_set_deterministic() {
    let subj = subject();
    let lexicon = assembly_lexicon();
    let state = state_with_edge_with_proof();

    let a = enumerate_placement_set(subj, &state, &empty_type_registry(), &lexicon);
    let b = enumerate_placement_set(subj, &state, &empty_type_registry(), &lexicon);

    assert_eq!(
        a.board_hash, b.board_hash,
        "same state ⇒ bit-identical hash"
    );
    let ids_a: Vec<&str> = a.moves.iter().map(|m| m.move_id.0.as_str()).collect();
    let ids_b: Vec<&str> = b.moves.iter().map(|m| m.move_id.0.as_str()).collect();
    assert_eq!(ids_a, ids_b, "same state ⇒ bit-identical move list");
}

// ── canonical_order_stable ─────────────────────────────────────────────────

#[test]
fn canonical_order_stable() {
    let subj = subject();
    let lexicon = assembly_lexicon();
    // Multiple edges inserted in an arbitrary order — BTreeMap<EdgeId, _> and
    // the placement set's own BTreeMap<MoveId, _> must still emit sorted output.
    let mut state = ControlState::default();
    for _ in 0..5 {
        let e = edge_with_proof();
        state.edges.insert(e.id, e);
    }

    let set = enumerate_placement_set(subj, &state, &empty_type_registry(), &lexicon);
    let ids: Vec<&str> = set.moves.iter().map(|m| m.move_id.0.as_str()).collect();
    let mut sorted = ids.clone();
    sorted.sort_unstable();
    assert_eq!(
        ids, sorted,
        "moves must be emitted in canonical (move_id) order"
    );
}

// ── placement_iff_precondition (differential oracle) ───────────────────────

/// For every lexicon entry and every candidate target the generator would
/// probe, the move is in the placement set iff `check_preconditions`
/// (the same oracle the write path enforces) accepts it against the state.
fn assert_matches_oracle(subj: SubjectId, state: &ControlState, lexicon: &LexiconManifest) {
    let set = enumerate_placement_set(subj, state, &empty_type_registry(), lexicon);

    for entry in lexicon.entries.values() {
        let fqn = entry.fqn.as_str();
        // D1 (EOP-DD-KYCUBO-TS.1 §3): the four type-registry moves are
        // entity-scoped (`TargetBinding.entity_id`), not subject-scoped —
        // this helper's generic per-entry probing only ever tries a single
        // bare-subject target, which `enumerate_placement_set` deliberately
        // never admits them against (`is_type_registry_move`,
        // `placement.rs`). Their real oracle/set equivalence is covered by
        // `ts1_assembly_board.rs`'s dedicated entity-scoped test instead.
        //
        // T2 (2026-08-27, §8 Q1): `register`+`type` MERGED into `place`
        // (still entity-scoped, still excluded here under its new name);
        // `member-withdrawal` renamed `remove` (same); `type-correction`
        // DISSOLVED — no lexicon entry remains for this loop to visit, so
        // its own exclusion-list entry is deleted, not left as dead cover.
        if matches!(
            fqn,
            "kyc_ubo.assert.subject.place"
                | "kyc_ubo.assert.subject.remove"
                | "kyc_ubo.assert.subject.enquiry"
        ) {
            continue;
        }
        // `kyc_ubo.assert.edge.nominee-piercing` dropped TS.6 P2 (K-G7) — retired,
        // folded into a macro; the loop this runs in only visits real
        // lexicon entries, so it never reaches that fqn.
        let is_edge_scoped = matches!(
            fqn,
            "kyc_ubo.assert.edge.evidence" | "kyc_ubo.assert.edge.disconnect"
        );
        let targets: Vec<TargetBinding> = if is_edge_scoped {
            state
                .edges
                .keys()
                .map(|eid| TargetBinding::for_edge(subj, *eid))
                .collect()
        } else {
            vec![TargetBinding::for_subject(subj)]
        };

        for target in targets {
            let probe = ob_poc_kyc_substrate::IntentEvent::new(
                subj,
                fqn,
                ob_poc_kyc_substrate::Principal::test_analyst(),
                ob_poc_kyc_substrate::AuthorityRef("oracle-probe".into()),
                target.clone(),
                serde_json::Value::Null,
                chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0).unwrap(),
            );
            let oracle_says_legal =
                check_preconditions(entry, state, &empty_type_registry(), &probe).is_ok();
            let set_says_legal = set.admits(fqn, &target);
            assert_eq!(
                oracle_says_legal, set_says_legal,
                "verb {fqn} target {target:?}: oracle={oracle_says_legal} set={set_says_legal}"
            );
        }
    }
}

#[test]
fn placement_iff_precondition_empty_state() {
    assert_matches_oracle(subject(), &empty_state(), &assembly_lexicon());
}

#[test]
fn placement_iff_precondition_asserted_edge() {
    assert_matches_oracle(
        subject(),
        &state_with_edge(EdgeStatus::Asserted),
        &assembly_lexicon(),
    );
}

#[test]
fn placement_iff_precondition_edge_with_proof() {
    // EOP-DD-UBO-PROOF-001 §4 (T5): `EdgeStatus` dropped its Evidenced/
    // Verified rungs — the only surviving richer-than-bare-Asserted board
    // state is an edge with a proof logged against it.
    assert_matches_oracle(
        subject(),
        &state_with_edge_with_proof(),
        &assembly_lexicon(),
    );
}

#[test]
fn placement_iff_precondition_reconciled_and_strategized() {
    assert_matches_oracle(
        subject(),
        &strategized_state(),
        &assembly_lexicon(),
    );
}

#[test]
fn placement_iff_precondition_reconciled_strategized_with_verified_edge() {
    let mut state = strategized_state();
    let e = edge_with_proof();
    state.edges.insert(e.id, e);
    assert_matches_oracle(subject(), &state, &assembly_lexicon());
}

// `verify_only_admitted_for_evidenced_or_verified_edges` RETIRED
// (EOP-DD-UBO-PROOF-001 §3/§4, T5, 2026-08-28) — `kyc_ubo.assert.edge.
// verification`, the precondition it exercised (`Precondition::
// EvidenceCited`), and the `EdgeStatus::Evidenced`/`Verified` ratchet it
// gated on are all deleted (K-G7: 0 real committed events under that FQN).
// There is no successor verb to test a "requires evidence first" gate
// for — `evidence` now logs a proof unconditionally.

// ── §14 Q2 benchmark ────────────────────────────────────────────────────────

/// Records the T2 cost benchmark (V&S §14 Q2): enumeration cost at a
/// realistic-to-generous edge count. Asserted as a loose SLA (not a strict
/// perf-regression gate — CI hardware varies) so a real slowdown still fails
/// the suite rather than silently drifting.
#[test]
fn enumerate_placement_set_scales_to_200_edges() {
    let subj = subject();
    let lexicon = assembly_lexicon();
    let mut state = ControlState::default();
    for i in 0..200 {
        // Half bare-Asserted, half with a proof logged — realistic mixed
        // board (EOP-DD-UBO-PROOF-001 §4, T5: `EdgeStatus` lost its
        // Evidenced/Verified rungs; a proof is now the richer axis).
        let e = if i % 2 == 0 { edge_with_proof() } else { edge(EdgeStatus::Asserted) };
        state.edges.insert(e.id, e);
    }

    let start = Instant::now();
    let set = enumerate_placement_set(subj, &state, &empty_type_registry(), &lexicon);
    let elapsed = start.elapsed();

    assert!(
        set.moves.len() > 200,
        "expect multiple candidate moves per edge across edge-scoped verbs"
    );
    assert!(
        elapsed.as_millis() < 200,
        "T2 §14 Q2 benchmark: enumeration over 200 edges took {elapsed:?}, expected <200ms"
    );
}

/// 2026-09-07 audit item 3 RED gate: `place` must offer entity TYPES
/// (EOP-VS-UBO-GAME-001 §3.4 R9 — "place offers the entity types that may
/// be added"; §2 — "the game writes metadata about entities, never
/// entities; which specific entity is a lookup"), not a pre-enumerated set
/// of already-known entity ids. On an empty board there is nothing yet to
/// connect to, so every catalogued type is a legal first placement — one
/// candidate per `EntityType` (21). Field-agnostic on purpose: this counts
/// `place` candidates rather than inspecting move shape, so it does not
/// presuppose P2's exact `LegalMove` field design. RED today: the current
/// generator emits exactly ONE `place` candidate (the bare subject-entity
/// id — `placement.rs`'s `place_and_remove_candidates`, T1's P4 deferral),
/// never a type.
#[test]
fn place_offers_entity_types() {
    let subj = subject();
    let lexicon = assembly_lexicon();
    let set = enumerate_placement_set(
        subj,
        &empty_state(),
        &empty_type_registry(),
        &lexicon,
    );
    let place_candidates = set
        .moves
        .iter()
        .filter(|m| m.verb_fqn.as_str() == "kyc_ubo.assert.subject.place")
        .count();
    assert_eq!(
        place_candidates, 21,
        "an empty board must offer all 21 catalogued entity types as `place` candidates \
         (TS.0 §4 catalogue) — got {place_candidates}. `place` candidates are type-level, \
         not a pre-enumerated set of known entity ids (a brand-new entity has no id yet \
         for the board to enumerate)"
    );
}
