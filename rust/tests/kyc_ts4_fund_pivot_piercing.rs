//! TS.4 gate tests — EOP-DD-KYCUBO-TS.4 (Fund Pivot Realisation and
//! Cross-Strategy Piercing, RATIFIED 2026-08-21). Realises TS.2's ratified
//! fund rulings (2a-2f) against the working chain TS.3 built, and moves
//! nominee piercing from a class strategy into the traversal itself
//! (TS.0 §5, K-8).
//!
//! Pure — no `DATABASE_URL`. Same direct-strategy-construction style as
//! `kyc_ts3_control_admission.rs`.
//!
//! **Phase 0 baseline (captured against the tree BEFORE any TS.4 code
//! change — see each `phase0_*` test's own assertions for the exact
//! pre-tranche behaviour).**

use std::collections::BTreeMap;
use std::collections::BTreeSet;

use ob_poc_kyc_substrate::{
    ControlProngStrategy, ControlState, CooperativeMemberStrategy, DelegationStatus,
    DeterminationStrategy, EdgeId, EdgeKind, EdgeState, EdgeStatus, EntityId, EventId,
    FoundationCouncilStrategy, FundControlStrategy, NomineePierceStrategy, OwnershipProngStrategy,
    PersonId, Prong, StateOwnedStrategy, TrustRoleStrategy, STRATEGY_DELEGATION_REGISTRY,
};

// ── Fixture builders ─────────────────────────────────────────────────────────

fn eid(tag: u128) -> EntityId {
    EntityId(uuid::Uuid::from_u128(0xF4_5E1F_0000_0000_0000_0000_0000 | tag))
}
fn evid(tag: u128) -> EventId {
    EventId(uuid::Uuid::from_u128(0xF4_5E1F_0000_0000_0000_0000_E000 | tag))
}

fn edge(id_tag: u128, kind: EdgeKind, from: EntityId, to: EntityId, orig_tag: u128) -> EdgeState {
    EdgeState {
        id: EdgeId(uuid::Uuid::from_u128(0xF4_5E1F_0000_0000_0000_0000_D000 | id_tag)),
        kind,
        from,
        to,
        percentage: None,
        status: EdgeStatus::Asserted,
        evidence_event_id: None,
        originating_event_id: evid(orig_tag),
        trust_revocable: None,
        superseded_by: None,
        pierced_from: None,
    }
}

// ── Phase 0a: baseline the fund-pivot rulings ────────────────────────────────

#[test]
fn phase0_fund_resolves_via_governing_mandate_but_is_a_plain_delegate() {
    // Fund <-ManagementMandate- ManCo <-VotingRights- Alice.
    let fund = eid(1);
    let manco = eid(2);
    let alice = PersonId(eid(3).0);
    let mut state = ControlState::default();
    let e1 = edge(1, EdgeKind::ManagementMandate, manco, fund, 1);
    let e2 = edge(2, EdgeKind::VotingRights, EntityId(alice.0), manco, 2);
    state.edges.insert(e1.id, e1);
    state.edges.insert(e2.id, e2);
    let natural_persons: BTreeSet<PersonId> = [alice].into_iter().collect();

    let fund_candidates = FundControlStrategy.resolve(&state, fund, &natural_persons, 25.0);
    let control_candidates = ControlProngStrategy.resolve(&state, fund, &natural_persons, 25.0);
    println!("PHASE0 fund_via_mandate (FundControlStrategy): {fund_candidates:#?}");
    println!("PHASE0 fund_via_mandate (ControlProngStrategy):  {control_candidates:#?}");

    // The chain resolves (TS.3 already fixed the walk) — but FundControlStrategy
    // is STILL a verbatim delegate: bit-identical to raw ControlProngStrategy,
    // proving there is no fund-specific provenance/pivot recording at all.
    assert_eq!(fund_candidates.len(), 1);
    assert_eq!(
        format!("{fund_candidates:?}"),
        format!("{control_candidates:?}"),
        "PRE-TS.4 baseline: FundControlStrategy must be indistinguishable from plain \
         ControlProngStrategy — the defect TS.4 §4 names"
    );
}

#[test]
fn phase0_co_management_second_manager_gets_no_smo_when_first_resolves() {
    // Fund managed by TWO ManCos: ManCo_A (resolves to Alice) and
    // ManCo_B (dead-ends, only an OfficerAppointment edge — should need its
    // own SMO pull, but doesn't get one because the exhaustion check is
    // global, not per-pivot).
    use ob_poc_kyc_substrate::{pull_smo_on_exhaustion, ProngCandidate};

    let fund = eid(4);
    let manco_a = eid(5);
    let manco_b = eid(6);
    let alice = PersonId(eid(7).0);
    let officer_b = PersonId(eid(8).0);
    let mut state = ControlState::default();
    let e1 = edge(3, EdgeKind::ManagementMandate, manco_a, fund, 3);
    let e2 = edge(4, EdgeKind::VotingRights, EntityId(alice.0), manco_a, 4);
    let e3 = edge(5, EdgeKind::ManagementMandate, manco_b, fund, 5);
    let e4 = edge(6, EdgeKind::OfficerAppointment, EntityId(officer_b.0), manco_b, 6);
    state.edges.insert(e1.id, e1);
    state.edges.insert(e2.id, e2);
    state.edges.insert(e3.id, e3);
    state.edges.insert(e4.id, e4);
    let natural_persons: BTreeSet<PersonId> = [alice, officer_b].into_iter().collect();

    let candidates = FundControlStrategy.resolve(&state, fund, &natural_persons, 25.0);
    println!("PHASE0 co_management (FundControlStrategy.resolve): {candidates:#?}");
    assert_eq!(candidates.len(), 1, "only Alice (via ManCo_A) resolves; ManCo_B's officer is invisible");
    assert_eq!(candidates[0].person_id, alice);

    // The GLOBAL exhaustion check (subject-anchored, candidates non-empty) skips
    // the pull entirely — officer_b never appears, silently, even though
    // ManCo_B's own chain genuinely exhausted.
    let global_pull = pull_smo_on_exhaustion(&state, fund, &natural_persons, &candidates);
    assert!(
        global_pull.is_none(),
        "PRE-TS.4: the global (subject-anchored) pull never fires once ANY candidate exists, \
         even though ManCo_B's branch independently exhausted: {global_pull:#?}"
    );
    let _: Vec<ProngCandidate> = vec![]; // (silences unused-import if the assertion above changes)
}

#[test]
fn phase0_mandate_without_evidence_still_freezes_today() {
    // A ManagementMandate edge with ZERO evidence (status Asserted, no
    // evidence_event_id) still lets the fund resolve — no evidence stud
    // exists at all today (Ruling 2f is entirely absent).
    let fund = eid(9);
    let manco = eid(10);
    let alice = PersonId(eid(11).0);
    let mut state = ControlState::default();
    let mandate = edge(7, EdgeKind::ManagementMandate, manco, fund, 7); // status: Asserted, no evidence
    let voting = edge(8, EdgeKind::VotingRights, EntityId(alice.0), manco, 8);
    assert!(mandate.evidence_event_id.is_none(), "fixture sanity: no contract evidence attached");
    state.edges.insert(mandate.id, mandate);
    state.edges.insert(voting.id, voting);
    let natural_persons: BTreeSet<PersonId> = [alice].into_iter().collect();

    let candidates = FundControlStrategy.resolve(&state, fund, &natural_persons, 25.0);
    println!("PHASE0 mandate_without_evidence: {candidates:#?}");
    assert_eq!(
        candidates.len(),
        1,
        "PRE-TS.4: an unevidenced mandate resolves exactly like an evidenced one — nothing \
         distinguishes them, and nothing would block a freeze on this basis today"
    );
}

// ── Phase 0b: THE ONE THAT MATTERS MOST — mid-chain nominee, unpierced ──────

#[test]
fn phase0b_mid_chain_nominee_inside_a_fund_is_not_pierced() {
    // Fund <-ManagementMandate- ManCo <-Nominee- Nominee_Corp.
    // The subject classifies as InvestmentFund (via FundControlStrategy),
    // NEVER as Nominee — NomineePierceStrategy is never selected, so its
    // freeze-site guard (scoped to `strategy_name == "nominee_pierce_strategy"`)
    // never runs for this determination.
    let fund = eid(12);
    let manco = eid(13);
    let nominee_corp = eid(14);
    let mut state = ControlState::default();
    let mandate = edge(9, EdgeKind::ManagementMandate, manco, fund, 9);
    // The ONLY edge into ManCo is a Nominee-kind edge — UNPIERCED (no
    // pierce-nominee event has run; no replacement edge exists).
    let nominee_edge = edge(10, EdgeKind::Nominee, nominee_corp, manco, 10);
    state.edges.insert(mandate.id, mandate);
    state.edges.insert(nominee_edge.id, nominee_edge.clone());
    let natural_persons: BTreeSet<PersonId> = BTreeSet::new();

    let candidates = FundControlStrategy.resolve(&state, fund, &natural_persons, 25.0);
    println!("PHASE0b mid_chain_nominee (candidates): {candidates:#?}");
    // Not pierced: the nominee edge is Pierce-classified, excluded from
    // reconciled_control_edges — ManCo's own resolution finds NOTHING.
    // The walk does not name the nominee, but it ALSO does not flag that
    // anything is wrong — it just silently produces zero candidates.
    assert!(
        candidates.is_empty(),
        "PRE-TS.4: the walk stops silently at the unpierced nominee, finding nothing: \
         {candidates:#?}"
    );

    // Worse: TS.3's exhaustion pull, run today exactly as it would be at
    // the live freeze site, happily falls back to SMOs of wherever the
    // walk dead-ended (ManCo) or the frontier including nominee_corp —
    // WITHOUT any awareness that an unresolved nominee arrangement sits
    // in the path. Nothing here errors, nothing flags the gap.
    use ob_poc_kyc_substrate::pull_smo_on_exhaustion;
    let pull = pull_smo_on_exhaustion(&state, fund, &natural_persons, &candidates);
    println!("PHASE0b mid_chain_nominee (smo pull, no officer edges present): {pull:#?}");
    assert!(
        pull.is_none(),
        "no OfficerAppointment edges exist in this fixture, so the pull itself finds nothing \
         to contribute — but nothing STOPS freeze from succeeding on this basis either: {pull:#?}"
    );

    // Confirm this is NOT a K-8 violation by omission alone — the nominee
    // arrangement really does exist in the graph, just invisible to the walk.
    assert_eq!(
        state
            .edges
            .values()
            .filter(|e| e.is_active() && matches!(e.kind, EdgeKind::Nominee))
            .count(),
        1,
        "sanity: exactly one active, unpierced Nominee edge sits mid-chain, unresolved"
    );
}

#[test]
fn phase0b_pierced_nominee_mid_chain_already_resolves_correctly() {
    // Contrast: if the SAME nominee arrangement HAS been pierced (the
    // replacement edge already exists, `pierced_from` set), the walk
    // already finds the real person today — piercing the SUBSTITUTION
    // itself is not broken; what's broken is (a) nothing RECORDS that a
    // pierce was followed, and (b) an UNPIERCED nominee anywhere outside
    // `nominee_pierce_strategy` is never checked at all (0b above).
    let fund = eid(15);
    let manco = eid(16);
    let nominee_corp = eid(17);
    let real_person = PersonId(eid(18).0);
    let mut state = ControlState::default();
    let mandate = edge(11, EdgeKind::ManagementMandate, manco, fund, 11);
    let mut nominee_edge = edge(12, EdgeKind::Nominee, nominee_corp, manco, 12);
    nominee_edge.status = EdgeStatus::Superseded;
    nominee_edge.superseded_by = Some(evid(13));
    let mut replacement = edge(13, EdgeKind::VotingRights, EntityId(real_person.0), manco, 13);
    replacement.pierced_from = Some(nominee_edge.id);
    state.edges.insert(mandate.id, mandate);
    state.edges.insert(nominee_edge.id, nominee_edge);
    state.edges.insert(replacement.id, replacement);
    let natural_persons: BTreeSet<PersonId> = [real_person].into_iter().collect();

    let candidates = FundControlStrategy.resolve(&state, fund, &natural_persons, 25.0);
    println!("PHASE0b pierced_nominee_mid_chain: {candidates:#?}");
    assert_eq!(candidates.len(), 1, "the replacement edge is an ordinary Traverse-admitted edge");
    assert_eq!(candidates[0].person_id, real_person);
    assert_eq!(candidates[0].prong, Prong::ControlByOtherMeans);
    // Confirmed: substitution already works. What's absent (checked in Phase 4)
    // is any RECORD that a pierce was followed to get here.
}

// ── NomineePierceStrategy baseline (for the delegation pin, Phase 1) ────────

#[test]
fn phase0_nominee_pierce_strategy_is_a_plain_delegate() {
    let subject = eid(19);
    let alice = PersonId(eid(20).0);
    let mut state = ControlState::default();
    let e = edge(14, EdgeKind::VotingRights, EntityId(alice.0), subject, 14);
    state.edges.insert(e.id, e);
    let natural_persons: BTreeSet<PersonId> = [alice].into_iter().collect();

    let nominee_candidates = NomineePierceStrategy.resolve(&state, subject, &natural_persons, 25.0);
    let control_candidates = ControlProngStrategy.resolve(&state, subject, &natural_persons, 25.0);
    assert_eq!(
        format!("{nominee_candidates:?}"),
        format!("{control_candidates:?}"),
        "NomineePierceStrategy is a plain delegate today, by design (Q1: this STAYS true \
         after TS.4 — a declared framing, not a defect)"
    );
}

// ── Phase 1 — the delegation pin (§4), RED-honest ────────────────────────────

#[test]
fn strategy_delegation_is_exactly_known() {
    let registry: BTreeMap<&str, DelegationStatus> =
        STRATEGY_DELEGATION_REGISTRY.iter().copied().collect();
    assert_eq!(registry.len(), 8, "8 distinct DeterminationStrategy impls exist today");

    // (a) exactly the declared delegates — named, not merely counted.
    let declared_delegates: BTreeSet<&str> = registry
        .iter()
        .filter_map(|(name, status)| {
            matches!(status, DelegationStatus::DeclaredDelegate { .. }).then_some(*name)
        })
        .collect();
    assert_eq!(
        declared_delegates,
        ["fund_control_strategy", "nominee_pierce_strategy"].into_iter().collect::<BTreeSet<_>>(),
        "TS.4 §4's finding: exactly these two strategies forward verbatim today. If this set \
         grows or shrinks, that is a real, conscious change to report, not silent drift."
    );

    // (b) behavioral proof: EVERY declared delegate really IS byte-identical
    // to its named target, across varied fixtures — an honest claim, not
    // just an unverified label.
    let subject_a = eid(102);
    let mut state_a = ControlState::default();
    let ea = edge(100, EdgeKind::VotingRights, eid(101), subject_a, 100);
    state_a.edges.insert(ea.id, ea);

    let subject_b = eid(105);
    let mut state_b = ControlState::default();
    let eb1 = edge(103, EdgeKind::BoardAppointment, eid(104), subject_b, 101);
    let eb2 = edge(106, EdgeKind::DominantInfluence, eid(107), subject_b, 102);
    state_b.edges.insert(eb1.id, eb1);
    state_b.edges.insert(eb2.id, eb2);

    let natural_persons: BTreeSet<PersonId> =
        [PersonId(eid(101).0), PersonId(eid(104).0), PersonId(eid(107).0)].into_iter().collect();

    let strategies: BTreeMap<&str, &dyn DeterminationStrategy> = [
        ("ownership_prong_strategy", &OwnershipProngStrategy as &dyn DeterminationStrategy),
        ("control_prong_strategy", &ControlProngStrategy as &dyn DeterminationStrategy),
        ("trust_role_strategy", &TrustRoleStrategy as &dyn DeterminationStrategy),
        ("fund_control_strategy", &FundControlStrategy as &dyn DeterminationStrategy),
        ("foundation_council_strategy", &FoundationCouncilStrategy as &dyn DeterminationStrategy),
        ("state_owned_strategy", &StateOwnedStrategy as &dyn DeterminationStrategy),
        ("cooperative_member_strategy", &CooperativeMemberStrategy as &dyn DeterminationStrategy),
        ("nominee_pierce_strategy", &NomineePierceStrategy as &dyn DeterminationStrategy),
    ]
    .into_iter()
    .collect();

    for (name, status) in &registry {
        if let DelegationStatus::DeclaredDelegate { delegates_to } = status {
            let this_strategy = strategies[name];
            let target_strategy = strategies[delegates_to];
            for (state, subject) in [(&state_a, subject_a), (&state_b, subject_b)] {
                let a = this_strategy.resolve(state, subject, &natural_persons, 25.0);
                let b = target_strategy.resolve(state, subject, &natural_persons, 25.0);
                assert_eq!(
                    format!("{a:?}"),
                    format!("{b:?}"),
                    "{name} is declared a delegate of {delegates_to} but diverges on subject \
                     {subject:?} — the pin is dishonest: either the delegation is fake or the \
                     registry is stale"
                );
            }
        }
    }

    // (c) spot-check: a GenuineImplementation with a narrower kind-filter
    // than control_prong_strategy really does diverge from it somewhere —
    // guards against "quietly became an undeclared delegate" in the other
    // direction too.
    let stray_subject = eid(108);
    let mut stray_state = ControlState::default();
    let stray_edge = edge(109, EdgeKind::MembershipRights, eid(110), stray_subject, 103);
    stray_state.edges.insert(stray_edge.id, stray_edge);
    let stray_persons: BTreeSet<PersonId> = [PersonId(eid(110).0)].into_iter().collect();
    let foundation_result =
        FoundationCouncilStrategy.resolve(&stray_state, stray_subject, &stray_persons, 25.0);
    let control_result =
        ControlProngStrategy.resolve(&stray_state, stray_subject, &stray_persons, 25.0);
    assert_ne!(
        format!("{foundation_result:?}"),
        format!("{control_result:?}"),
        "foundation_council_strategy is registered as GenuineImplementation — it must diverge \
         from control_prong_strategy somewhere (its own narrower kind filter), or the registry \
         entry is suspect: foundation={foundation_result:#?} control={control_result:#?}"
    );
}
