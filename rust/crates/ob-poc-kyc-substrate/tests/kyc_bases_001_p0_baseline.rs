//! EOP-DD-UBO-BASES-001 P0 — recon + baseline.
//!
//! Pure — no `DATABASE_URL`. Builds a minimal, deterministic board (fixed
//! UUIDs via `eid`/`evid`, mirroring `tests/kyc_t4_dispatch.rs`'s fixture
//! style) for every `EntityType` that dispatches to a real strategy today
//! (19 of 21 — `NaturalPerson`/`SoleTrader` are terminal), resolves it
//! through TODAY's single-strategy dispatch, and prints the full candidate
//! set. This is §0b's before-half: P5 re-runs the same 19 boards after the
//! P2-P4 changes land and diffs the printed output — every type except
//! `PrivateLimitedCompany`/`PublicListedCompany` must be byte-identical.
//!
//! Run with `cargo test -p ob-poc-kyc-substrate --test kyc_bases_001_p0_baseline -- --nocapture`.

use std::collections::{BTreeMap, BTreeSet};

use ob_poc_kyc_substrate::{
    ControlProngStrategy, ControlState, CooperativeMemberStrategy, DeterminationDispatch,
    DeterminationStrategy, EdgeId, EdgeKind, EdgeState, EdgeStatus, EntityId, EntityType,
    EventId, FoundationCouncilStrategy, FundControlStrategy, OwnershipProngStrategy, PersonId,
    StateOwnedStrategy, TrustRoleKind, TrustRoleStrategy,
};

// ── Fixture builders (mirrors tests/kyc_t4_dispatch.rs) ─────────────────────

fn eid(tag: u128) -> EntityId {
    EntityId(uuid::Uuid::from_u128(0xBA5E_0000_0000_0000_0000_0000_0000 | tag))
}
fn evid(tag: u128) -> EventId {
    EventId(uuid::Uuid::from_u128(0xBA5E_0000_0000_0000_0000_0000_E000 | tag))
}
fn edgeid(tag: u128) -> EdgeId {
    EdgeId(uuid::Uuid::from_u128(0xBA5E_0000_0000_0000_0000_0000_D000 | tag))
}

fn control_edge(tag: u128, kind: EdgeKind, from: EntityId, to: EntityId) -> EdgeState {
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

fn economic_edge(tag: u128, from: EntityId, to: EntityId, pct: f64) -> EdgeState {
    EdgeState {
        id: edgeid(tag),
        kind: EdgeKind::EconomicInterest,
        from,
        to,
        percentage: Some(pct),
        status: EdgeStatus::Asserted,
        proofs: BTreeMap::new(),
        originating_event_id: evid(tag),
        trust_revocable: None,
        superseded_by: None,
        pierced_from: None,
    }
}

fn strategy_for(name: &str) -> &'static dyn DeterminationStrategy {
    match name {
        "ownership_prong_strategy" => &OwnershipProngStrategy,
        "control_prong_strategy" => &ControlProngStrategy,
        "trust_role_strategy" => &TrustRoleStrategy,
        "fund_control_strategy" => &FundControlStrategy,
        "foundation_council_strategy" => &FoundationCouncilStrategy,
        "state_owned_strategy" => &StateOwnedStrategy,
        "cooperative_member_strategy" => &CooperativeMemberStrategy,
        other => panic!("kyc_bases_001_p0_baseline: unknown strategy name {other}"),
    }
}

/// One board per `EntityType` that has a strategy: a subject of that type,
/// one natural-person controller, one edge of a kind valid for that type's
/// dispatched strategy. Returns (type, board label, resolved candidates).
fn board_for(t: EntityType, tag: u128) -> (EntityType, Vec<ob_poc_kyc_substrate::ProngCandidate>) {
    let subject = eid(tag * 100);
    let person = PersonId(eid(tag * 100 + 1).0);
    let natural_persons: BTreeSet<PersonId> = [person].into_iter().collect();
    let mut state = ControlState::default();

    use EntityType::*;
    match t {
        PrivateLimitedCompany | PublicListedCompany => {
            let e = economic_edge(tag, EntityId(person.0), subject, 40.0);
            state.edges.insert(e.id, e);
        }
        LlcUs => {
            let e = control_edge(tag, EdgeKind::VotingRights, EntityId(person.0), subject);
            state.edges.insert(e.id, e);
        }
        GeneralPartnership | LimitedPartnership => {
            let e = control_edge(tag, EdgeKind::GpStatutory, EntityId(person.0), subject);
            state.edges.insert(e.id, e);
        }
        Llp => {
            let e = control_edge(tag, EdgeKind::DesignatedMember, EntityId(person.0), subject);
            state.edges.insert(e.id, e);
        }
        OeicIcvc | Sicav | UnitTrust | FortyActFund | UmbrellaWithSubFunds => {
            // Governing-mandate pivot held directly by the natural person.
            let e = control_edge(tag, EdgeKind::ManagementMandate, EntityId(person.0), subject);
            state.edges.insert(e.id, e);
        }
        LpFund => {
            let e = control_edge(tag, EdgeKind::GpStatutory, EntityId(person.0), subject);
            state.edges.insert(e.id, e);
        }
        DiscretionaryTrust | FixedBareTrust | PensionScheme | CharityNotForProfit | Foundation => {
            let e = control_edge(
                tag,
                EdgeKind::TrustRole(TrustRoleKind::Trustee),
                EntityId(person.0),
                subject,
            );
            state.edges.insert(e.id, e);
        }
        CooperativeMutual => {
            let e = control_edge(tag, EdgeKind::MembershipRights, EntityId(person.0), subject);
            state.edges.insert(e.id, e);
        }
        GovernmentDeptStatutoryCorporation => {
            let e = control_edge(tag, EdgeKind::DominantInfluence, EntityId(person.0), subject);
            state.edges.insert(e.id, e);
        }
        NaturalPerson | SoleTrader => unreachable!("terminal types have no strategy"),
    }

    let strategy_names = match ob_poc_kyc_substrate::dispatch_for_entity_type(&t) {
        DeterminationDispatch::Strategies(names) => names,
        DeterminationDispatch::NotADeterminationSubject => {
            unreachable!("{t:?} is terminal, not iterated here")
        }
    };
    // Mirrors freeze's real orchestration (EOP-DD-UBO-BASES-001 §3): every
    // strategy in the dispatch set runs and the results union. This
    // fixture only ever carries ONE relevant edge, so for every type this
    // produces the SAME candidates as calling the single historically-
    // dispatched strategy alone — the point of the P0/P5 diff is exactly
    // that this widening is a no-op when there's nothing new to find.
    let mut candidates = Vec::new();
    for name in strategy_names {
        candidates.extend(strategy_for(name).resolve(&state, subject, &natural_persons, 25.0));
    }
    candidates.sort_by_key(|c| c.person_id.0);
    (t, candidates)
}

const ALL_STRATEGY_TYPES: &[EntityType] = &[
    EntityType::PrivateLimitedCompany,
    EntityType::PublicListedCompany,
    EntityType::LlcUs,
    EntityType::GeneralPartnership,
    EntityType::LimitedPartnership,
    EntityType::Llp,
    EntityType::OeicIcvc,
    EntityType::Sicav,
    EntityType::UnitTrust,
    EntityType::FortyActFund,
    EntityType::LpFund,
    EntityType::UmbrellaWithSubFunds,
    EntityType::DiscretionaryTrust,
    EntityType::FixedBareTrust,
    EntityType::Foundation,
    EntityType::PensionScheme,
    EntityType::CooperativeMutual,
    EntityType::CharityNotForProfit,
    EntityType::GovernmentDeptStatutoryCorporation,
];

#[test]
fn baseline_boards_19_types() {
    assert_eq!(ALL_STRATEGY_TYPES.len(), 19, "19 types dispatch to a real strategy today");
    for (tag, t) in ALL_STRATEGY_TYPES.iter().enumerate() {
        let (t, candidates) = board_for(*t, tag as u128 + 1);
        eprintln!("=== {t:?} ===");
        for c in &candidates {
            eprintln!(
                "  person={} prong={:?} pct={:?} chain_len={} orig={} pivot={} pierces={}",
                c.person_id.0,
                c.prong,
                c.effective_ownership_pct,
                c.ownership_chain.len(),
                c.originating_event_id.0,
                c.pivot.is_some(),
                c.pierces.len()
            );
        }
        if candidates.is_empty() {
            eprintln!("  <no candidates>");
        }
    }
}
