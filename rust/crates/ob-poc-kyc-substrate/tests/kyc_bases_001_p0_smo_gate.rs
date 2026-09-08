//! EOP-DD-UBO-BASES-001 §5/§7 `smo_fallback_fires_only_on_total_exhaustion`
//! — RED via a real assertion failure (not a compile error): TODAY, an LLC
//! whose only edge is an officer appointment resolves that officer through
//! the SMO PULL (`Prong::SmoFallback`), not through direct control
//! admission (`Prong::ControlByOtherMeans`) — because
//! `control_admission(OfficerAppointment) == NotControl` (pre-R-A).
//! `LlcUs` already dispatches to `control_prong_strategy` today
//! (DISPATCH-001 §2 row 5, unaffected by BASES-001 §3), so this fixture
//! needs no dispatch-table change to demonstrate the gap — only R-A
//! (§5: `OfficerAppointment` → `Traverse`) changes which mechanism finds
//! the officer. Post-R-A: `control_prong_strategy` finds them directly as
//! `ControlByOtherMeans`; the SMO pull never fires at all (its own
//! exhaustion guard sees a non-empty `prior_candidates`).

use std::collections::{BTreeMap, BTreeSet};

use ob_poc_kyc_substrate::{
    control_admission, pull_smo_on_exhaustion, ControlAdmission, ControlProngStrategy,
    ControlState, DeterminationStrategy, EdgeId, EdgeKind, EdgeState, EdgeStatus, EntityId,
    EventId, PersonId, Prong,
};

fn eid(tag: u128) -> EntityId {
    EntityId(uuid::Uuid::from_u128(0xBA5E_0002_0000_0000_0000_0000_0000 | tag))
}
fn evid(tag: u128) -> EventId {
    EventId(uuid::Uuid::from_u128(0xBA5E_0002_0000_0000_0000_0000_E000 | tag))
}
fn edgeid(tag: u128) -> EdgeId {
    EdgeId(uuid::Uuid::from_u128(0xBA5E_0002_0000_0000_0000_0000_D000 | tag))
}

#[test]
fn smo_fallback_fires_only_on_total_exhaustion() {
    let llc = eid(1);
    let officer = PersonId(eid(2).0);
    let natural_persons: BTreeSet<PersonId> = [officer].into_iter().collect();

    let mut state = ControlState::default();
    let officer_edge = EdgeState {
        id: edgeid(1),
        kind: EdgeKind::OfficerAppointment,
        from: EntityId(officer.0),
        to: llc,
        percentage: None,
        status: EdgeStatus::Asserted,
        proofs: BTreeMap::new(),
        originating_event_id: evid(1),
        trust_revocable: None,
        superseded_by: None,
        pierced_from: None,
    };
    state.edges.insert(officer_edge.id, officer_edge);

    // R-A landed: OfficerAppointment is now Traverse-admitted.
    assert_eq!(control_admission(&EdgeKind::OfficerAppointment), ControlAdmission::Traverse);

    // The ruled target (R-A, §5): the officer is a DOOR — direct control
    // admission must find them, labelled `ControlByOtherMeans`, without
    // ever needing the separate SMO-pull scan. Fails today: `NotControl`
    // means `control_prong_strategy` sees nothing, so this is empty.
    let direct = ControlProngStrategy.resolve(&state, llc, &natural_persons, 25.0);
    assert_eq!(
        direct.len(),
        1,
        "EOP-DD-UBO-BASES-001 §5 R-A: OfficerAppointment must become Traverse-admitted so \
         control_prong_strategy finds the officer directly, not via the separate SMO-pull \
         scan — today's NotControl classification makes this empty: {direct:#?}"
    );
    assert_eq!(
        direct[0].prong,
        Prong::ControlByOtherMeans,
        "the officer is a genuine control candidate, not a fallback-of-last-resort"
    );

    // The corollary, R-B: once direct admission finds the officer,
    // `prior_candidates` is non-empty, so the SMO pull's own exhaustion
    // guard must refuse to fire for this board at all — the two
    // mechanisms are mutually exclusive by construction, never both
    // contributing the same person.
    let smo_pull = pull_smo_on_exhaustion(&state, llc, &natural_persons, &direct);
    assert!(
        smo_pull.is_none(),
        "R-B: the SMO pull must not fire once the officer is already admitted directly — got \
         {smo_pull:?}"
    );
}
