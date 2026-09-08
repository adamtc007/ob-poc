//! EOP-DD-UBO-BASES-001 P4 regression — the frontier-computation fix inside
//! `pull_smo_on_exhaustion` that R-A's `control_admission` widening made
//! necessary (see the function's own doc and the state-of-play §7 entry).
//!
//! `CharityNotForProfit` dispatches to `trust_role_strategy` (§2 Q3
//! RULED), which reads ONLY `TrustRole`-kind edges — it never sees an
//! `OfficerAppointment` edge regardless of R-A. TS.1's geometry table
//! admits `OfficerAppointment`/`EmploymentDelegatedAuthority` onto a
//! `CharityNotForProfit` target (`ts1_assembly_board.rs`'s
//! `expected_target_pipes`), so a charity with an officer and NO trustee
//! is a real, geometrically-legal board: `trust_role_strategy` finds
//! nothing (`prior_candidates` empty), and the officer must still be
//! rescued by `pull_smo_on_exhaustion`'s independent frontier scan.
//!
//! Before the P4 fix, R-A's widening of `reconciled_control_edges` broke
//! exactly this case: the frontier walk saw the officer as a
//! natural-person parent of the subject and silently dropped the subject
//! from `frontier` (instead of correctly classifying it as exhausted-here),
//! so the officer scan found nothing and the function wrongly returned
//! `None` — turning a legitimate SMO-pull rescue into a K-5 hard refusal.

use std::collections::{BTreeMap, BTreeSet};

use ob_poc_kyc_substrate::{
    pull_smo_on_exhaustion, ControlState, DeterminationDispatch, DeterminationStrategy, EdgeId,
    EdgeKind, EdgeState, EdgeStatus, EntityId, EntityType, EventId, PersonId, Prong,
    TrustRoleStrategy,
};

fn eid(tag: u128) -> EntityId {
    EntityId(uuid::Uuid::from_u128(0xBA5E_0003_0000_0000_0000_0000_0000 | tag))
}
fn evid(tag: u128) -> EventId {
    EventId(uuid::Uuid::from_u128(0xBA5E_0003_0000_0000_0000_0000_E000 | tag))
}
fn edgeid(tag: u128) -> EdgeId {
    EdgeId(uuid::Uuid::from_u128(0xBA5E_0003_0000_0000_0000_0000_D000 | tag))
}

#[test]
fn officer_on_a_charity_with_no_trustee_is_still_rescued_by_the_smo_pull() {
    assert_eq!(
        ob_poc_kyc_substrate::dispatch_for_entity_type(&EntityType::CharityNotForProfit),
        DeterminationDispatch::Strategies(&["trust_role_strategy"]),
        "sanity: CharityNotForProfit dispatches to trust_role_strategy alone"
    );

    let charity = eid(1);
    let officer = PersonId(eid(2).0);
    let natural_persons: BTreeSet<PersonId> = [officer].into_iter().collect();

    let mut state = ControlState::default();
    let officer_edge = EdgeState {
        id: edgeid(1),
        kind: EdgeKind::OfficerAppointment,
        from: EntityId(officer.0),
        to: charity,
        percentage: None,
        status: EdgeStatus::Asserted,
        proofs: BTreeMap::new(),
        originating_event_id: evid(1),
        trust_revocable: None,
        superseded_by: None,
        pierced_from: None,
    };
    state.edges.insert(officer_edge.id, officer_edge);

    // trust_role_strategy never sees the officer — no TrustRole edge exists.
    let trust_candidates = TrustRoleStrategy.resolve(&state, charity, &natural_persons, 25.0);
    assert!(
        trust_candidates.is_empty(),
        "trust_role_strategy must find nothing on a charity with no trust-role edge: \
         {trust_candidates:#?}"
    );

    // The SMO pull must still rescue the officer — this is the regression
    // R-A's widening of reconciled_control_edges introduced and P4 fixed.
    let pulled = pull_smo_on_exhaustion(&state, charity, &natural_persons, &trust_candidates)
        .expect("the officer must still be rescued via the exhaustion pull");
    assert_eq!(pulled.0.len(), 1, "exactly the officer must be pulled: {pulled:#?}");
    assert_eq!(pulled.0[0].person_id, officer);
    assert_eq!(pulled.0[0].prong, Prong::SmoFallback);
    assert_eq!(pulled.0[0].bases.len(), 1);
    assert_eq!(pulled.0[0].bases[0].edge_kind, EdgeKind::OfficerAppointment);
}
