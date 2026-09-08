//! EOP-DD-UBO-BASES-001 §7 `candidate_carries_every_admitting_basis` and
//! `disconnect_removes_a_route_not_the_person` — RED via compiler error
//! (same discipline as `kyc_bases_001_p0_prong_dual.rs` and the pattern
//! `tests/kyc_t4_dispatch.rs` documents: "first a compile error... then a
//! real assertion failure once [the change] lands"). `ProngCandidate` has
//! no `bases` field and `AdmittingBasis` does not exist yet — §4's ruled
//! shape. P3 adds both; this file then compiles and its assertions become
//! the real gate.

use std::collections::{BTreeMap, BTreeSet};

use ob_poc_kyc_substrate::{
    AdmittingBasis, ControlProngStrategy, ControlState, DeterminationStrategy, EdgeId, EdgeKind,
    EdgeState, EdgeStatus, EntityId, EventId, PersonId,
};

fn eid(tag: u128) -> EntityId {
    EntityId(uuid::Uuid::from_u128(0xBA5E_0001_0000_0000_0000_0000_0000 | tag))
}
fn evid(tag: u128) -> EventId {
    EventId(uuid::Uuid::from_u128(0xBA5E_0001_0000_0000_0000_0000_E000 | tag))
}
fn edgeid(tag: u128) -> EdgeId {
    EdgeId(uuid::Uuid::from_u128(0xBA5E_0001_0000_0000_0000_0000_D000 | tag))
}

fn ctrl_edge(tag: u128, kind: EdgeKind, from: EntityId, to: EntityId) -> EdgeState {
    EdgeState {
        id: edgeid(tag),
        kind,
        from,
        to,
        percentage: None,
        status: EdgeStatus::Asserted,
        proofs: BTreeMap::new(),
        originating_event_id: evid(tag),
        trust_revocable: None,
        superseded_by: None,
        pierced_from: None,
    }
}

/// Company controlled by ONE person through THREE independent bases —
/// voting rights, a board seat, and dominant influence. All three are
/// simultaneously true and each is independently sufficient (§1).
fn three_basis_fixture() -> (EntityId, PersonId, ControlState, [EdgeId; 3]) {
    let company = eid(1);
    let person = PersonId(eid(2).0);
    let mut state = ControlState::default();

    let e1 = ctrl_edge(1, EdgeKind::VotingRights, EntityId(person.0), company);
    let e2 = ctrl_edge(2, EdgeKind::BoardAppointment, EntityId(person.0), company);
    let e3 = ctrl_edge(3, EdgeKind::DominantInfluence, EntityId(person.0), company);
    let ids = [e1.id, e2.id, e3.id];
    state.edges.insert(e1.id, e1);
    state.edges.insert(e2.id, e2);
    state.edges.insert(e3.id, e3);

    (company, person, state, ids)
}

#[test]
fn candidate_carries_every_admitting_basis() {
    let (company, person, state, edge_ids) = three_basis_fixture();
    let natural_persons: BTreeSet<PersonId> = [person].into_iter().collect();

    let candidates = ControlProngStrategy.resolve(&state, company, &natural_persons, 25.0);
    assert_eq!(
        candidates.len(),
        1,
        "admitted by 3 edges must still be ONE candidate, not 3: {candidates:#?}"
    );
    let bases: &Vec<AdmittingBasis> = &candidates[0].bases;
    assert_eq!(
        bases.len(),
        3,
        "the candidate must carry all 3 admitting bases named: {bases:#?}"
    );
    let recorded_edge_ids: BTreeSet<EdgeId> = bases.iter().map(|b| b.edge_id).collect();
    let expected_edge_ids: BTreeSet<EdgeId> = edge_ids.into_iter().collect();
    assert_eq!(recorded_edge_ids, expected_edge_ids, "every admitting edge id must be named");

    let recorded_kinds: BTreeSet<String> =
        bases.iter().map(|b| format!("{:?}", b.edge_kind)).collect();
    assert_eq!(
        recorded_kinds,
        [
            format!("{:?}", EdgeKind::VotingRights),
            format!("{:?}", EdgeKind::BoardAppointment),
            format!("{:?}", EdgeKind::DominantInfluence),
        ]
        .into_iter()
        .collect::<BTreeSet<_>>(),
        "every admitting edge kind must be named"
    );
}

/// §7 `disconnect_removes_a_route_not_the_person` — the fuzz invariant,
/// promoted: N=3 independent bases; remove N-1=2; the person survives with
/// the ONE remaining basis correctly named (not the wrong survivor, not a
/// vanished candidate).
#[test]
fn disconnect_removes_a_route_not_the_person() {
    let (company, person, mut state, edge_ids) = three_basis_fixture();
    let natural_persons: BTreeSet<PersonId> = [person].into_iter().collect();

    // Remove N-1 = 2 of the 3 bases — supersede voting_rights and
    // board_appointment, leaving only dominant_influence active. Mirrors
    // how `disconnect` is folded (superseded, not deleted) — see
    // `fold::control`'s `EdgeStatus::Superseded` handling.
    if let Some(e) = state.edges.get_mut(&edge_ids[0]) {
        e.status = EdgeStatus::Superseded;
    }
    if let Some(e) = state.edges.get_mut(&edge_ids[1]) {
        e.status = EdgeStatus::Superseded;
    }

    let candidates = ControlProngStrategy.resolve(&state, company, &natural_persons, 25.0);
    assert_eq!(
        candidates.len(),
        1,
        "the person must survive with 2 of 3 bases withdrawn: {candidates:#?}"
    );
    let bases = &candidates[0].bases;
    assert_eq!(bases.len(), 1, "exactly the ONE surviving basis must be named: {bases:#?}");
    assert_eq!(
        bases[0].edge_id, edge_ids[2],
        "the named survivor must be dominant_influence (the one edge left active)"
    );
    assert_eq!(bases[0].edge_kind, EdgeKind::DominantInfluence);
}
