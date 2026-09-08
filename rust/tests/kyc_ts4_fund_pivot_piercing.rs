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
//! change).** Three of the original phase0 tests
//! (`phase0_fund_resolves_via_governing_mandate_but_is_a_plain_delegate`,
//! `phase0_co_management_second_manager_gets_no_smo_when_first_resolves`,
//! `strategy_delegation_is_exactly_known`'s declared-delegate set) pinned
//! exactly the defect Ruling A (§2) fixes; once Phase 2's `fund_pivot_resolve`
//! landed they necessarily went RED (an INTENDED difference, not drift — see
//! each test's comment) and are rewritten in place below as the Phase 2 gate
//! tests they were always going to become, named per TS.4 §5. The two
//! `phase0b_*` mid-chain-nominee tests and `phase0_nominee_pierce_strategy_
//! is_a_plain_delegate` describe `DeterminationStrategy::resolve()`'s OWN
//! behaviour, which Ruling B does NOT change (the substitution already
//! happens at edge-admission time — TS.0's `pierce-nominee` verb; Ruling B's
//! fix is the freeze-dispatch guard widening + pierce recording, both at
//! the op layer / `recover_determination_at`, exercised in
//! `kyc_ts4_pierce_traversal.rs`'s live-DB gates) — so they remain accurate
//! and unchanged. `pierce_cycle_terminates` (bottom of this file) is the one
//! Ruling B gate that IS pure (no DB needed).

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
        proofs: BTreeMap::new(),
        originating_event_id: evid(orig_tag),
        trust_revocable: None,
        superseded_by: None,
        pierced_from: None,
    }
}

// ── Phase 0a: baseline the fund-pivot rulings ────────────────────────────────

#[test]
fn pivot_is_recorded_with_basis() {
    // Fund <-ManagementMandate- ManCo <-VotingRights- Alice.
    // PRE-Ruling-A this was `phase0_fund_resolves_via_governing_mandate_
    // but_is_a_plain_delegate`, asserting FundControlStrategy was bit-
    // identical to a raw ControlProngStrategy walk — exactly the "claimed-
    // but-delegated" defect TS.4 §4 names. Ruling A fixes it: the
    // determination now RECORDS the pivot (entity + basis edge kind +
    // mandate edge id), so it diverges from the raw walk by construction.
    let fund = eid(1);
    let manco = eid(2);
    let alice = PersonId(eid(3).0);
    let mut state = ControlState::default();
    let e1 = edge(1, EdgeKind::ManagementMandate, manco, fund, 1);
    let e2 = edge(2, EdgeKind::VotingRights, EntityId(alice.0), manco, 2);
    let mandate_edge_id = e1.id;
    state.edges.insert(e1.id, e1);
    state.edges.insert(e2.id, e2);
    let natural_persons: BTreeSet<PersonId> = [alice].into_iter().collect();

    let fund_candidates = FundControlStrategy.resolve(&state, fund, &natural_persons, 25.0);
    let control_candidates = ControlProngStrategy.resolve(&state, fund, &natural_persons, 25.0);
    println!("fund_via_mandate (FundControlStrategy): {fund_candidates:#?}");
    println!("fund_via_mandate (ControlProngStrategy):  {control_candidates:#?}");

    assert_eq!(fund_candidates.len(), 1);
    assert_ne!(
        format!("{fund_candidates:?}"),
        format!("{control_candidates:?}"),
        "Ruling A: FundControlStrategy must now diverge from a raw ControlProngStrategy walk — \
         it carries a recorded pivot the raw walk cannot express"
    );

    let candidate = &fund_candidates[0];
    assert_eq!(candidate.person_id, alice);
    let pivot = candidate.pivot.as_ref().expect("Ruling A: every fund candidate carries its pivot");
    assert_eq!(pivot.pivot_entity, manco, "the determination names the pivot entity");
    assert_eq!(
        pivot.basis_edge_kind,
        EdgeKind::ManagementMandate,
        "Ruling 2: basis, not the label 'ManCo'"
    );
    assert_eq!(pivot.mandate_edge_id, mandate_edge_id);
    assert!(!pivot.mandate_evidenced, "fixture mandate is bare Asserted — not yet evidenced (2f)");
    assert_eq!(
        candidate.ownership_chain,
        vec![fund, manco],
        "K-1/K-35: the chain reads fund -> pivot (terminal natural person is not itself pushed \
         onto the chain, matching resolve_chain_candidates' existing convention)"
    );
}

#[test]
fn co_management_unions_with_per_pivot_paths() {
    // Fund managed by TWO ManCos: ManCo_A resolves to Alice directly;
    // ManCo_B dead-ends into only an OfficerAppointment edge. PRE-Ruling-A
    // (`phase0_co_management_second_manager_gets_no_smo_when_first_
    // resolves`) the GLOBAL, subject-anchored exhaustion check skipped the
    // pull entirely once ANY candidate existed, so ManCo_B's officer was
    // silently invisible. Ruling 2a/2e fix this: exhaustion is per-pivot
    // (anchored at each ManCo independently), so co-management is a UNION —
    // neither branch suppresses the other.
    use ob_poc_kyc_substrate::ProngCandidate;

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
    println!("co_management (FundControlStrategy.resolve): {candidates:#?}");
    assert_eq!(
        candidates.len(),
        2,
        "Ruling 2a/2e: BOTH ManCo_A's Alice and ManCo_B's per-pivot-pulled officer resolve"
    );

    let by_person: BTreeMap<PersonId, &ProngCandidate> =
        candidates.iter().map(|c| (c.person_id, c)).collect();

    let alice_c = by_person[&alice];
    assert_eq!(alice_c.prong, Prong::ControlByOtherMeans);
    assert_eq!(
        alice_c.pivot.as_ref().expect("Alice carries her pivot").pivot_entity,
        manco_a,
        "Alice's own pivot path names ManCo_A, not suppressed by ManCo_B's branch"
    );

    let officer_c = by_person[&officer_b];
    // EOP-DD-UBO-BASES-001 §5 R-A (2026-09-08): OfficerAppointment is now
    // Traverse-admitted, so ManCo_B's own per-pivot control-chain walk finds
    // the officer directly (ControlByOtherMeans) before any exhaustion pull
    // is ever attempted — the per-pivot union property this test names
    // still holds (both branches independently contribute), only the
    // MECHANISM that surfaces ManCo_B's officer changed, not the union
    // itself.
    assert_eq!(
        officer_c.prong,
        Prong::ControlByOtherMeans,
        "R-A: ManCo_B's own pivot walk finds the officer directly, no pull needed"
    );
    assert_eq!(
        officer_c.pivot.as_ref().expect("officer_b carries its pivot").pivot_entity,
        manco_b,
        "exhaustion targets the PIVOT (ManCo_B), not the fund"
    );
}

#[test]
fn exhaustion_pulls_smo_of_pivot_entity() {
    // Fund <-ManagementMandate- ManCo <-OfficerAppointment- Carol.
    //
    // EOP-DD-UBO-BASES-001 §5 R-A (2026-09-08): at TS.4 landing time, no
    // Traverse-admitted control-kind edge reached any natural person here,
    // so ManCo's own chain exhausted immediately and Ruling 2a's per-pivot
    // exhaustion pull fired, anchored at ManCo. R-A reclassifies
    // OfficerAppointment to Traverse: ManCo's own control-chain walk now
    // finds Carol DIRECTLY (ControlByOtherMeans) — the walk no longer
    // exhausts at all, so the pull this test originally named never fires
    // for this fixture. What survives is the anchoring property (still
    // pivot-scoped, not fund-scoped), now demonstrated by direct admission
    // rather than by the pull.
    let fund = eid(25);
    let manco = eid(26);
    let carol = PersonId(eid(27).0);
    let mut state = ControlState::default();
    let mandate = edge(18, EdgeKind::ManagementMandate, manco, fund, 18);
    let officer = edge(19, EdgeKind::OfficerAppointment, EntityId(carol.0), manco, 19);
    state.edges.insert(mandate.id, mandate);
    state.edges.insert(officer.id, officer);
    let natural_persons: BTreeSet<PersonId> = [carol].into_iter().collect();

    let candidates = FundControlStrategy.resolve(&state, fund, &natural_persons, 25.0);
    println!("exhaustion_pulls_smo_of_pivot_entity: {candidates:#?}");
    assert_eq!(candidates.len(), 1, "ManCo's own control chain finds Carol directly");
    assert_eq!(candidates[0].person_id, carol);
    assert_eq!(
        candidates[0].prong,
        Prong::ControlByOtherMeans,
        "R-A: OfficerAppointment is Traverse-admitted; ManCo's own pivot walk finds Carol \
         directly, no pull needed"
    );
    assert_eq!(
        candidates[0].pivot.as_ref().expect("carries its pivot").pivot_entity,
        manco,
        "resolution is still anchored at ManCo (the pivot entity), never at the fund"
    );
}

#[test]
fn gp_of_lp_resolves_via_general_partner() {
    // LP <-GpStatutory- GP <-VotingRights- Bob; limited partners hold
    // EconomicInterest (investor issuance) directly into the LP. Ruling 2:
    // basis, not the label "ManCo" — an LP resolves through its GP exactly
    // the way a corporate-form fund resolves through its ManCo/AIFM, and
    // limited partners (economic axis) appear in NO control determination.
    let lp = eid(21);
    let gp = eid(22);
    let bob = PersonId(eid(23).0);
    let limited_partner = eid(24);
    let mut state = ControlState::default();
    let gp_edge = edge(15, EdgeKind::GpStatutory, gp, lp, 15);
    let bob_edge = edge(16, EdgeKind::VotingRights, EntityId(bob.0), gp, 16);
    let lp_economic = edge(17, EdgeKind::EconomicInterest, limited_partner, lp, 17);
    state.edges.insert(gp_edge.id, gp_edge);
    state.edges.insert(bob_edge.id, bob_edge);
    state.edges.insert(lp_economic.id, lp_economic);
    let natural_persons: BTreeSet<PersonId> =
        [bob, PersonId(limited_partner.0)].into_iter().collect();

    let candidates = FundControlStrategy.resolve(&state, lp, &natural_persons, 25.0);
    println!("gp_of_lp_resolves_via_general_partner: {candidates:#?}");
    assert_eq!(candidates.len(), 1, "the LP resolves through its GP, basis not label");
    assert_eq!(candidates[0].person_id, bob);
    assert_eq!(
        candidates[0].pivot.as_ref().expect("carries its pivot").basis_edge_kind,
        EdgeKind::GpStatutory,
        "the pivot basis is GpStatutory — not a ManagementMandate label"
    );
    assert!(
        candidates.iter().all(|c| c.person_id != PersonId(limited_partner.0)),
        "limited partners appear in NO control determination — economic_interest stays on the \
         economic axis, never traversed by the control-axis fund pivot: {candidates:#?}"
    );
}

#[test]
fn pivot_cycle_terminates_and_records() {
    // A degenerate circular management arrangement: the fund's own
    // governing-mandate edge names itself as its manager. Ruling 2b/2d:
    // the branch halts (never admits the fund as its own controller) and
    // the halt is RECORDED, not silently dropped.
    use ob_poc_kyc_substrate::fund_pivot_resolve;

    let fund = eid(28);
    let mut state = ControlState::default();
    let self_mandate = edge(20, EdgeKind::ManagementMandate, fund, fund, 20);
    state.edges.insert(self_mandate.id, self_mandate);
    let natural_persons: BTreeSet<PersonId> = BTreeSet::new();

    let result = fund_pivot_resolve(&state, fund, &natural_persons);
    println!("pivot_cycle_terminates_and_records: {result:#?}");
    assert!(result.candidates.is_empty(), "the self-referential pivot admits no controller");
    assert_eq!(result.cycles.len(), 1, "the cycle is recorded, not silently dropped");
    assert_eq!(result.cycles[0].revisited_entity, fund);
    assert_eq!(result.cycles[0].pivot_path, vec![fund]);
}

#[test]
fn no_im_specific_path_exists() {
    // Structural check (TS.4 §2c/2d): an "affiliated IM" relationship that
    // is NOT itself a governing-mandate edge (ManagementMandate/GpStatutory)
    // finds no pivot at all here — it falls through to ordinary economic
    // (ownership ) ) traversal. There is no third "delegated IM" EdgeKind
    // anywhere in the taxonomy for fund_pivot_resolve to branch on.
    use ob_poc_kyc_substrate::EDGE_KIND_WIRE_VALUES;

    // (a) the wire vocabulary itself has no IM-specific kind — only the
    // ratified control/economic taxonomy (`governing_mandate_edges_into`
    // admits exactly `management_mandate`/`gp_statutory`, nothing else).
    assert!(
        !EDGE_KIND_WIRE_VALUES.contains(&"instrument_matrix")
            && !EDGE_KIND_WIRE_VALUES.contains(&"im_delegate")
            && !EDGE_KIND_WIRE_VALUES.contains(&"im_mandate"),
        "no IM-specific EdgeKind exists in the wire vocabulary: {EDGE_KIND_WIRE_VALUES:?}"
    );

    // (b) behaviourally: a fund affiliated only via plain EconomicInterest
    // (no governing mandate at all) is invisible to fund_pivot_resolve's
    // control-axis pivot search and resolves ONLY via ordinary ownership
    // traversal (OwnershipProngStrategy), never via any IM-specific branch.
    let fund = eid(29);
    let affiliate = eid(30);
    let alice = PersonId(eid(31).0);
    let mut state = ControlState::default();
    let mut affiliation = edge(21, EdgeKind::EconomicInterest, affiliate, fund, 21);
    affiliation.percentage = Some(100.0);
    let mut ownership = edge(22, EdgeKind::EconomicInterest, EntityId(alice.0), affiliate, 22);
    ownership.percentage = Some(100.0);
    state.edges.insert(affiliation.id, affiliation);
    state.edges.insert(ownership.id, ownership);
    let natural_persons: BTreeSet<PersonId> = [alice].into_iter().collect();

    let fund_control_result = FundControlStrategy.resolve(&state, fund, &natural_persons, 25.0);
    assert!(
        fund_control_result.is_empty(),
        "no governing-mandate edge exists — the control-axis fund pivot finds nothing: \
         {fund_control_result:#?}"
    );
    let ownership_result = OwnershipProngStrategy.resolve(&state, fund, &natural_persons, 25.0);
    assert_eq!(
        ownership_result.len(),
        1,
        "the affiliated case resolves through ordinary ownership traversal instead"
    );
    assert_eq!(ownership_result[0].person_id, alice);
}

#[test]
fn phase0_mandate_without_evidence_still_freezes_today() {
    // A ManagementMandate edge with ZERO proofs (status Asserted, empty
    // `proofs`) still lets the fund resolve — no evidence stud exists at
    // all today (Ruling 2f is entirely absent).
    let fund = eid(9);
    let manco = eid(10);
    let alice = PersonId(eid(11).0);
    let mut state = ControlState::default();
    let mandate = edge(7, EdgeKind::ManagementMandate, manco, fund, 7); // status: Asserted, no proofs
    let voting = edge(8, EdgeKind::VotingRights, EntityId(alice.0), manco, 8);
    assert!(!mandate.has_proof(), "fixture sanity: no contract evidence attached");
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
        ["nominee_pierce_strategy"].into_iter().collect::<BTreeSet<_>>(),
        "post-Ruling-A: fund_control_strategy is now a GenuineImplementation (fund_pivot_resolve); \
         nominee_pierce_strategy remains a DECLARED delegate by design (Q1 — stays true through \
         Ruling B, which moves the MECHANISM into the walk but keeps the name as a framing). If \
         this set grows or shrinks beyond that, that is a real, conscious change to report, not \
         silent drift."
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

// ── Phase 3 — pierce cycle termination (Ruling B), the one pure gate ───────

#[test]
fn pierce_cycle_terminates() {
    // subject <-VotingRights- manco (ordinary edge), manco <-VotingRights-
    // subject (a REPLACEMENT edge from a prior pierce — `pierced_from` set
    // — that happens to close a cycle back to the subject itself). Proves
    // the existing path-based cycle guard in `resolve_chain_candidates`
    // applies uniformly whether or not an edge in the loop is a pierce
    // replacement — a pierced chain that loops still terminates and admits
    // no phantom candidate.
    let subject = eid(35);
    let manco = eid(36);
    let nominee = eid(37);
    let mut state = ControlState::default();
    let e1 = edge(23, EdgeKind::VotingRights, manco, subject, 23);
    let mut nominee_edge = edge(24, EdgeKind::Nominee, nominee, manco, 24);
    nominee_edge.status = EdgeStatus::Superseded;
    nominee_edge.superseded_by = Some(evid(25));
    let mut replacement = edge(25, EdgeKind::VotingRights, subject, manco, 25);
    replacement.pierced_from = Some(nominee_edge.id);
    state.edges.insert(e1.id, e1);
    state.edges.insert(nominee_edge.id, nominee_edge);
    state.edges.insert(replacement.id, replacement);
    let natural_persons: BTreeSet<PersonId> = BTreeSet::new();

    // Must terminate (this test process itself not hanging IS the proof)
    // and admit no candidate — subject and manco only chase each other; no
    // natural person is ever reached.
    let candidates = ControlProngStrategy.resolve(&state, subject, &natural_persons, 25.0);
    assert!(candidates.is_empty(), "cycle through a pierce replacement admits no candidate: {candidates:#?}");
}
