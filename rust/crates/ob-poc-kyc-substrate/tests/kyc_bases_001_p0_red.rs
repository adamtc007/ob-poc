//! EOP-DD-UBO-BASES-001 §7 RED gates provable against TODAY's code (no new
//! API needed — these fail on a real assertion, not a compile error).
//! Pure — no `DATABASE_URL`.
//!
//! Fixture, shared by both gates: company `C` has TWO natural persons —
//! `shareholder` holds 30% shares (clears the 25% threshold, an ordinary
//! ownership-prong candidate) and `officer` holds the office of MD
//! (`EdgeKind::OfficerAppointment`) with ZERO shares. Both are real,
//! independent, simultaneously-true facts about the SAME company (§1).
//!
//! `resolve_via_real_dispatch` mirrors `kyc_stream_ops.rs`'s real freeze
//! orchestration (dispatch → union every strategy in the set → SMO pull on
//! total exhaustion) so these gates track the ACTUAL fix as it lands in
//! phases, rather than freezing a snapshot of today's specific defect:
//! P0 (today): dispatch is ownership-only, `OfficerAppointment` is
//! `NotControl` — RED. P2 (dispatch returns a set): companies also run
//! `control_prong_strategy`, but it still can't see `OfficerAppointment`
//! (still `NotControl`) — still RED. P4 (R-A: `OfficerAppointment` →
//! `Traverse`): `control_prong_strategy` finds the officer directly — GREEN.

use std::collections::{BTreeMap, BTreeSet};

use ob_poc_kyc_substrate::{
    pull_smo_on_exhaustion, ControlProngStrategy, ControlState, DeterminationDispatch,
    DeterminationStrategy, EdgeId, EdgeKind, EdgeState, EdgeStatus, EntityId, EntityType,
    EventId, OwnershipProngStrategy, PersonId, ProngCandidate,
};

fn eid(tag: u128) -> EntityId {
    EntityId(uuid::Uuid::from_u128(0xBA5E_0000_0000_0000_0000_0000_0000 | tag))
}
fn evid(tag: u128) -> EventId {
    EventId(uuid::Uuid::from_u128(0xBA5E_0000_0000_0000_0000_0000_E000 | tag))
}
fn edgeid(tag: u128) -> EdgeId {
    EdgeId(uuid::Uuid::from_u128(0xBA5E_0000_0000_0000_0000_0000_D000 | tag))
}

/// Returns (company, shareholder_person, officer_person, state).
fn officer_and_shareholder_fixture() -> (EntityId, PersonId, PersonId, ControlState) {
    let company = eid(9001);
    let shareholder = PersonId(eid(9002).0);
    let officer = PersonId(eid(9003).0);
    let mut state = ControlState::default();

    let economic = EdgeState {
        id: edgeid(1),
        kind: EdgeKind::EconomicInterest,
        from: EntityId(shareholder.0),
        to: company,
        percentage: Some(30.0),
        status: EdgeStatus::Asserted,
        proofs: BTreeMap::new(),
        originating_event_id: evid(1),
        trust_revocable: None,
        superseded_by: None,
        pierced_from: None,
    };
    let officer_edge = EdgeState {
        id: edgeid(2),
        kind: EdgeKind::OfficerAppointment,
        from: EntityId(officer.0),
        to: company,
        percentage: None,
        status: EdgeStatus::Asserted,
        proofs: BTreeMap::new(),
        originating_event_id: evid(2),
        trust_revocable: None,
        superseded_by: None,
        pierced_from: None,
    };
    state.edges.insert(economic.id, economic);
    state.edges.insert(officer_edge.id, officer_edge);
    (company, shareholder, officer, state)
}

fn strategy_for(name: &str) -> &'static dyn DeterminationStrategy {
    match name {
        "ownership_prong_strategy" => &OwnershipProngStrategy,
        "control_prong_strategy" => &ControlProngStrategy,
        other => panic!("kyc_bases_001_p0_red: unexpected strategy {other} for this fixture"),
    }
}

/// Mirrors `kyc_stream_ops.rs`'s real freeze orchestration for a
/// `PrivateLimitedCompany` subject: dispatch → run every strategy in the
/// set → union → SMO pull only on total exhaustion of the union.
fn resolve_via_real_dispatch(
    company: EntityId,
    natural_persons: &BTreeSet<PersonId>,
    state: &ControlState,
) -> Vec<ProngCandidate> {
    let names = match ob_poc_kyc_substrate::dispatch_for_entity_type(&EntityType::PrivateLimitedCompany) {
        DeterminationDispatch::Strategies(names) => names,
        DeterminationDispatch::NotADeterminationSubject => {
            unreachable!("PrivateLimitedCompany is always a determination subject")
        }
    };
    let mut candidates: Vec<ProngCandidate> = Vec::new();
    for name in names {
        candidates.extend(strategy_for(name).resolve(state, company, natural_persons, 25.0));
    }
    if let Some((pulled, _)) = pull_smo_on_exhaustion(state, company, natural_persons, &candidates) {
        candidates.extend(pulled);
    }
    candidates
}

/// §7: "an ordinary company with a controlling officer and no qualifying
/// shareholder resolves the officer. Fails today: the officer is
/// invisible." (Stronger form here: a DIFFERENT person qualifies by
/// shareholding, proving the officer is suppressed by ANOTHER candidate's
/// presence, not merely absent from an empty result.)
#[test]
fn company_dispatch_runs_both_limbs() {
    let (company, shareholder, officer, state) = officer_and_shareholder_fixture();
    let natural_persons: BTreeSet<PersonId> = [shareholder, officer].into_iter().collect();

    let result = resolve_via_real_dispatch(company, &natural_persons, &state);
    let ids: BTreeSet<PersonId> = result.iter().map(|c| c.person_id).collect();

    assert!(
        ids.contains(&shareholder),
        "the shareholder must always be resolved: {result:#?}"
    );
    assert!(
        ids.contains(&officer),
        "EOP-DD-UBO-BASES-001 §0/§3/§7 company_dispatch_runs_both_limbs: the officer must be \
         resolved alongside the shareholder via the real freeze dispatch — got: {result:#?}"
    );
}

/// §7 `officer_is_a_door_not_a_fallback`: "an MD who is also a minor
/// shareholder is admitted by the officer basis regardless of whether the
/// shareholding clears threshold." Uses the SAME fixture, driven through
/// the same real-dispatch helper.
#[test]
fn officer_is_a_door_not_a_fallback() {
    let (company, shareholder, officer, state) = officer_and_shareholder_fixture();
    let natural_persons: BTreeSet<PersonId> = [shareholder, officer].into_iter().collect();

    let ownership_candidates =
        OwnershipProngStrategy.resolve(&state, company, &natural_persons, 25.0);
    assert!(
        !ownership_candidates.iter().any(|c| c.person_id == officer),
        "the officer holds ZERO shares — ownership_prong_strategy must not resolve them by \
         economic weight; the officer basis must be a SEPARATE door, not folded into the \
         ownership prong"
    );

    let result = resolve_via_real_dispatch(company, &natural_persons, &state);
    let ids: BTreeSet<PersonId> = result.iter().map(|c| c.person_id).collect();
    assert!(
        ids.contains(&officer),
        "EOP-DD-UBO-BASES-001 §7 officer_is_a_door_not_a_fallback: the officer must be admitted \
         by their own basis regardless of the shareholding threshold — got: {result:#?}"
    );
}
