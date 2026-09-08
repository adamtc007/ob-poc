//! TS.3 gate tests — EOP-DD-KYCUBO-TS.3 (Control Admission and Prong
//! Semantics, RATIFIED 2026-08-21). Reconciles the `c7ef69ca` whitelist
//! against V&S v0.6 §6.3/§6.4/§9.2: replaces the boolean
//! `is_admitted_as_control` with the four-way `ControlAdmission`
//! (`Traverse | Stop | NotControl | Pierce`), admits `ManagementMandate`
//! and `MembershipRights` to `Traverse`, makes `StatutoryAuthority` a
//! recorded `Stop`, and implements SMO-pull-on-exhaustion (§4a) plus
//! traversal-decision provisionality (§2a).
//!
//! **This tranche deliberately changes determination outputs** — the first
//! non-equivalence-preserving change in the D1 corrective programme. Gates
//! here assert the INTENDED difference, not sameness (except
//! `employment_and_containment_never_traversed`/`unit_issuance_never_
//! confers_control`, where §3 itself rules "no change" — equivalence is
//! the CORRECT assertion there, not the forbidden kind).
//!
//! Pure — no `DATABASE_URL`. Mirrors `tests/kyc_pack_closure.rs`'s
//! direct-strategy-construction style (`ControlState::default()` + manual
//! edge insertion) — the semantics under test live entirely in
//! `ob-poc-kyc-substrate`, which is deliberately DB-free.
//!
//! **Phase 0 baseline (captured against the tree BEFORE any TS.3 code
//! change, this tranche's own receipt):**
//! ```text
//! PHASE0 fund_with_only_management_mandate: []
//! PHASE0 cooperative_with_membership_rights: []
//! PHASE0 state_body_via_statutory_authority: []
//! PHASE0 employment_and_containment (with stray edges):    [Alice, ControlByOtherMeans]
//! PHASE0 employment_and_containment (without stray edges): [Alice, ControlByOtherMeans]  (identical)
//! ```
//! The fund/co-op/state-body cases produced NOTHING pre-TS.3 — no control
//! path at all. `fund_with_manco_now_resolves`/`cooperative_resolves_via_
//! membership` below are the "after" half of that same proof for the two
//! that TS.3 rules should change; `statutory_authority_stops_with_reason`
//! is the "after" half for the one that stays empty but stops recording
//! why; the employment/containment case was already inert and stays so.

use std::collections::{BTreeMap, BTreeSet};

use ob_poc_kyc_substrate::{
    compute_assurance, control_admission, detect_statutory_stops, pull_smo_on_exhaustion,
    ControlAdmission, ControlProngStrategy, ControlState, CooperativeMemberStrategy,
    DeterminationStrategy, EdgeId, EdgeKind, EdgeState, EdgeStatus, EntityId, EntityType,
    EntityTypeRecord, EventId, FundControlStrategy, OwnershipProngStrategy, PersonId, ProofKind,
    ProofRecord, Prong, ProvisionalityReason, StateOwnedStrategy, StructureClass,
    TypeRegistryState, EDGE_KIND_WIRE_VALUES,
};

// ── Fixture builders ─────────────────────────────────────────────────────────

fn eid(tag: u128) -> EntityId {
    EntityId(uuid::Uuid::from_u128(0xF3_5E1F_0000_0000_0000_0000_0000 | tag))
}
fn evid(tag: u128) -> EventId {
    EventId(uuid::Uuid::from_u128(0xF3_5E1F_0000_0000_0000_0000_E000 | tag))
}

fn edge(id_tag: u128, kind: EdgeKind, from: EntityId, to: EntityId, orig_tag: u128) -> EdgeState {
    EdgeState {
        id: EdgeId(uuid::Uuid::from_u128(0xF3_5E1F_0000_0000_0000_0000_D000 | id_tag)),
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

/// EOP-DD-UBO-PROOF-001 §4 (T5): isolates the TYPE provisionality signal
/// from the EDGE one — a proof-bearing edge never triggers `UncitedEdge`,
/// the same role `edge_with_status(e, EdgeStatus::Verified)` played before
/// the ratchet was retired.
fn edge_with_proof(mut e: EdgeState, orig_tag: u128) -> EdgeState {
    let citing = evid(orig_tag);
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

/// EOP-DD-UBO-PROOF-001 §4 (T5): `has_proof` replaces the old `TypeProofStatus`
/// parameter — `false` == the old `Alleged`, `true` == the old `Proved`.
fn type_record(entity_type: EntityType, has_proof: bool, orig_tag: u128) -> EntityTypeRecord {
    let citing = evid(orig_tag);
    let mut proofs = BTreeMap::new();
    if has_proof {
        proofs.insert(
            citing,
            ProofRecord {
                kind: ProofKind::FiledDocument,
                source: "test fixture".to_string(),
                date: "2026-08-28".to_string(),
                event_id: citing,
            },
        );
    }
    EntityTypeRecord { entity_type, originating_event_id: citing, proofs }
}

// ── Phase 0 baseline receipt lives in the module doc comment above ──────────

// ── §7 gate: fund_with_manco_now_resolves (the headline) ────────────────────

#[test]
fn fund_with_manco_now_resolves() {
    // Fund <-ManagementMandate- ManCo <-BoardAppointment- Alice(natural person).
    let fund = eid(1);
    let manco = eid(2);
    let alice = PersonId(eid(3).0);
    let mut state = ControlState {
        structure_class: Some(StructureClass::InvestmentFund),
        ..Default::default()
    };
    let e1 = edge(1, EdgeKind::ManagementMandate, manco, fund, 1);
    let e2 = edge(2, EdgeKind::BoardAppointment, EntityId(alice.0), manco, 2);
    state.edges.insert(e1.id, e1);
    state.edges.insert(e2.id, e2);
    let natural_persons: BTreeSet<PersonId> = [alice].into_iter().collect();

    let candidates = FundControlStrategy.resolve(&state, fund, &natural_persons, 25.0);
    assert_eq!(
        candidates.len(),
        1,
        "PRE-TS.3 this was empty (see module doc Phase 0 receipt) — a fund whose ONLY \
         control path is a management mandate must now produce a determination: {candidates:#?}"
    );
    assert_eq!(candidates[0].person_id, alice);
    assert_eq!(candidates[0].prong, Prong::ControlByOtherMeans);
    assert_eq!(candidates[0].effective_ownership_pct, None, "control carries no quantum (K-3)");
    assert_eq!(candidates[0].ownership_chain, vec![fund, manco]);
}

// ── §7 gate: cooperative_resolves_via_membership ─────────────────────────────

#[test]
fn cooperative_resolves_via_membership() {
    // Coop <-MembershipRights- Bob(natural person). No board/voting edges;
    // "economic % often meaningless" (V&S §6.4) — membership itself is the
    // control axis (TS.3 §3).
    let coop = eid(4);
    let bob = PersonId(eid(5).0);
    let mut state = ControlState {
        structure_class: Some(StructureClass::Cooperative),
        ..Default::default()
    };
    let e1 = edge(3, EdgeKind::MembershipRights, EntityId(bob.0), coop, 3);
    state.edges.insert(e1.id, e1);
    let natural_persons: BTreeSet<PersonId> = [bob].into_iter().collect();

    let candidates = CooperativeMemberStrategy.resolve(&state, coop, &natural_persons, 25.0);
    assert_eq!(
        candidates.len(),
        1,
        "PRE-TS.3 this was empty (see module doc Phase 0 receipt) — a co-op with \
         membership-rights edges must now resolve via control: {candidates:#?}"
    );
    assert_eq!(candidates[0].person_id, bob);
    assert_eq!(candidates[0].prong, Prong::ControlByOtherMeans);
    assert_eq!(candidates[0].effective_ownership_pct, None);
}

// ── §7 gate: statutory_authority_stops_with_reason ───────────────────────────

#[test]
fn statutory_authority_stops_with_reason() {
    // A state body <-StatutoryAuthority- GovtDept. (Previously named for
    // `SovereignWealthVehicle`; removed T4, EOP-DD-UBO-DISPATCH-001 §2
    // Q4/§3a — not a single legal entity, never a block. `swv` kept as the
    // variable name; the fixture now types the stopped-at entity as
    // `GovernmentDeptStatutoryCorporation`, the one remaining catalogue
    // type this geometry cell still permits.)
    let swv = eid(6);
    let govt = eid(7);
    let mut state = ControlState {
        structure_class: Some(StructureClass::StateOwned),
        ..Default::default()
    };
    let e1 = edge(4, EdgeKind::StatutoryAuthority, govt, swv, 4);
    state.edges.insert(e1.id, e1.clone());
    let natural_persons: BTreeSet<PersonId> = BTreeSet::new();

    // Does not walk onward and does not eagerly contribute officials.
    let candidates = StateOwnedStrategy.resolve(&state, swv, &natural_persons, 25.0);
    assert!(candidates.is_empty(), "a Stop must not produce a candidate: {candidates:#?}");

    // But it DOES record why — no longer a silent exclusion.
    let stops = detect_statutory_stops(&state, swv);
    assert_eq!(stops.len(), 1, "exactly one statutory-authority stop expected: {stops:#?}");
    assert_eq!(stops[0].stopped_at, swv);
    assert_eq!(stops[0].authority_entity, govt);
    assert!(!stops[0].reason.is_empty(), "the stop must record a non-empty reason");
    assert_eq!(stops[0].originating_event_id, e1.originating_event_id);
}

// ── §7 gate: officers_contribute_only_on_exhaustion ──────────────────────────
//
// SUPERSEDED BY EOP-DD-UBO-BASES-001 §5 R-A (2026-09-08, recorded in
// `docs/eop/EOP-STATE-KYCUBO-D1_State-of-Play.md` §7, not by editing this
// ratified TS.3 gate): at TS.3 landing time `OfficerAppointment` was
// `NotControl` — an officer only ever surfaced via the separate
// SMO-pull-on-exhaustion mechanism, so adding one to an already-resolving
// structure was provably inert (the old assertion this test's name
// promised). R-A reclassifies `OfficerAppointment`/`Employment` to
// `Traverse` (§1: "control has several doors, any one opens") — an
// officer is now an independently-sufficient door for THEIR OWN person,
// so adding an officer edge to a resolving structure now DOES add the
// officer as their own candidate; it still changes nothing for the
// PERSON already resolving via a different basis (independent doors,
// independent people — `merge_candidates_by_person` only unions bases
// for the SAME person). Renamed to state the surviving property
// accurately — THIS test (not a file elsewhere; a prior version of this
// comment pointed to `kyc_bases_001_p0_smo_gate.rs`, which does not exist,
// found and corrected in the EOP-DD-UBO-BASES-001 audit closure tranche,
// 2026-09-08) IS the "officer is a door, not a fallback" positive gate
// this test's old name promised. See `employment_is_a_door_not_a_fallback`
// below for the sibling gate over R-A's other admitted kind.
#[test]
fn officer_is_an_independent_door_not_gated_on_exhaustion() {
    let subject = eid(8);
    let alice = PersonId(eid(9).0); // resolves via VotingRights
    let carol = PersonId(eid(10).0); // an officer — now an independent door too

    // (a) Ownership/control already resolves via VotingRights.
    let mut resolving = ControlState { structure_class: None, ..Default::default() };
    let voting = edge(5, EdgeKind::VotingRights, EntityId(alice.0), subject, 5);
    resolving.edges.insert(voting.id, voting);
    let natural_persons: BTreeSet<PersonId> = [alice, carol].into_iter().collect();

    let candidates_without_officer =
        ControlProngStrategy.resolve(&resolving, subject, &natural_persons, 25.0);
    assert_eq!(candidates_without_officer.len(), 1);
    assert_eq!(candidates_without_officer[0].person_id, alice);

    // Now add an OfficerAppointment edge for Carol at the SAME subject —
    // R-A: Carol now appears too, as her OWN independent candidate; Alice's
    // candidate is unaffected (independent doors, independent people).
    let officer = edge(6, EdgeKind::OfficerAppointment, EntityId(carol.0), subject, 6);
    resolving.edges.insert(officer.id, officer);
    let candidates_with_officer =
        ControlProngStrategy.resolve(&resolving, subject, &natural_persons, 25.0);
    assert_eq!(
        candidates_with_officer.len(),
        2,
        "R-A: the officer is now an independent door, not gated on exhaustion: \
         {candidates_with_officer:#?}"
    );
    assert!(candidates_with_officer.iter().any(|c| c.person_id == alice));
    assert!(candidates_with_officer.iter().any(|c| c.person_id == carol
        && c.prong == Prong::ControlByOtherMeans));
    assert!(
        pull_smo_on_exhaustion(&resolving, subject, &natural_persons, &candidates_with_officer)
            .is_none(),
        "the pull must not fire when prior_candidates is non-empty"
    );

    // (b) Contrast retained: with NO VotingRights edge, the SAME officer
    // edge alone now resolves DIRECTLY (Traverse) — never via the SMO pull.
    let mut exhausted = ControlState { structure_class: None, ..Default::default() };
    let officer2 = edge(7, EdgeKind::OfficerAppointment, EntityId(carol.0), subject, 7);
    exhausted.edges.insert(officer2.id, officer2);
    let direct = ControlProngStrategy.resolve(&exhausted, subject, &natural_persons, 25.0);
    assert_eq!(direct.len(), 1, "R-A: the officer resolves directly, no pull needed: {direct:#?}");
    assert_eq!(direct[0].person_id, carol);
    assert_eq!(
        direct[0].prong,
        Prong::ControlByOtherMeans,
        "R-A: direct admission, not the separate SmoFallback prong"
    );
    assert!(
        pull_smo_on_exhaustion(&exhausted, subject, &natural_persons, &direct).is_none(),
        "the pull must not fire once the primary walk already found the officer directly"
    );
}

// ── EOP-DD-UBO-BASES-001 closure tranche P3c (2026-09-08, audit item 9) ──────
//
// The positive covering gate for `EdgeKind::Employment` as its own
// independently-sufficient door — the mirror of
// `officer_is_an_independent_door_not_gated_on_exhaustion` above, but for
// R-A's SECOND admitted kind. Before this test, nothing anywhere positively
// drove `EdgeKind::Employment` admitting a candidate: `admission_is_exhaustive`
// and the wire-value closure tooth only pin COUNTS (13 traverse kinds, 2
// not-control), and `containment_never_traversed`'s own doc comment claimed
// this coverage existed "alongside `officer_is_an_independent_door...`"
// above — it did not; that test only ever constructs `OfficerAppointment`
// edges. Adam's actual words, quoted in the law: "employment with delegated
// authority is the thing" — this is that thing, driven.
#[test]
fn employment_is_a_door_not_a_fallback() {
    let subject = eid(50);
    let alice = PersonId(eid(51).0); // resolves via VotingRights
    let dana = PersonId(eid(52).0); // an employee with delegated authority — a door of her own

    // (a) Ownership/control already resolves via VotingRights. Adding a
    // Employment edge for Dana at the SAME subject must add Dana as her OWN
    // independent candidate; Alice's candidate must be unaffected.
    let mut resolving = ControlState { structure_class: None, ..Default::default() };
    let voting = edge(60, EdgeKind::VotingRights, EntityId(alice.0), subject, 60);
    resolving.edges.insert(voting.id, voting);
    let natural_persons: BTreeSet<PersonId> = [alice, dana].into_iter().collect();

    let candidates_without_employee =
        ControlProngStrategy.resolve(&resolving, subject, &natural_persons, 25.0);
    assert_eq!(candidates_without_employee.len(), 1);
    assert_eq!(candidates_without_employee[0].person_id, alice);

    let employment = edge(61, EdgeKind::Employment, EntityId(dana.0), subject, 61);
    resolving.edges.insert(employment.id, employment);
    let candidates_with_employee =
        ControlProngStrategy.resolve(&resolving, subject, &natural_persons, 25.0);
    assert_eq!(
        candidates_with_employee.len(),
        2,
        "R-A: employment-with-delegated-authority is an independent door, not gated on \
         exhaustion: {candidates_with_employee:#?}"
    );
    assert!(candidates_with_employee.iter().any(|c| c.person_id == alice));
    assert!(candidates_with_employee
        .iter()
        .any(|c| c.person_id == dana && c.prong == Prong::ControlByOtherMeans));
    assert!(
        pull_smo_on_exhaustion(&resolving, subject, &natural_persons, &candidates_with_employee)
            .is_none(),
        "the pull must not fire when prior_candidates is non-empty"
    );

    // (b) Contrast: with NO VotingRights edge, the SAME employment edge
    // alone resolves DIRECTLY (Traverse) — never via the SMO pull, never
    // labelled SmoFallback.
    let mut exhausted = ControlState { structure_class: None, ..Default::default() };
    let employment2 = edge(62, EdgeKind::Employment, EntityId(dana.0), subject, 62);
    exhausted.edges.insert(employment2.id, employment2);
    let direct = ControlProngStrategy.resolve(&exhausted, subject, &natural_persons, 25.0);
    assert_eq!(
        direct.len(),
        1,
        "R-A: employment resolves directly, no pull needed: {direct:#?}"
    );
    assert_eq!(direct[0].person_id, dana);
    assert_eq!(
        direct[0].prong,
        Prong::ControlByOtherMeans,
        "R-A: direct admission via Employment, not the separate SmoFallback prong"
    );
    assert!(
        pull_smo_on_exhaustion(&exhausted, subject, &natural_persons, &direct).is_none(),
        "the pull must not fire once the primary walk already found the employee directly"
    );
    assert_eq!(control_admission(&EdgeKind::Employment), ControlAdmission::Traverse);
}

// ── §7 gate: smo_fallback_is_recorded_never_silent ───────────────────────────

#[test]
fn smo_fallback_is_recorded_never_silent() {
    // Fund -ManagementMandate-> nothing but ManCo, ManCo has ONLY an
    // OfficerAppointment edge (no natural-person control edge) — ownership
    // and control both exhaust at ManCo (TS.2 Ruling 2a: the mandate
    // holder, not the fund).
    //
    // **Superseded by EOP-DD-KYCUBO-TS.4 §2 Ruling A (2026-08-22)** and then
    // again by **EOP-DD-UBO-BASES-001 §5 R-A (2026-09-08)**, both INTENDED
    // differences, not drift: at TS.3 landing time, `FundControlStrategy`
    // was still a thin delegate to `ControlProngStrategy`, so its own walk
    // exhausted with ZERO candidates and an EXTERNAL, subject-anchored
    // `pull_smo_on_exhaustion` call was needed to surface the officer.
    // TS.4's `fund_pivot_resolve` made the per-pivot exhaustion pull
    // INTERNAL to `FundControlStrategy` itself (anchored at the pivot), so
    // `resolve()` alone returned the officer directly — but still labelled
    // `Prong::SmoFallback`, because `OfficerAppointment` was still
    // `NotControl` and the ONLY route to the officer was the pull. R-A
    // reclassifies `OfficerAppointment` to `Traverse`: the pivot's own
    // control-chain walk (`resolve_chain_candidates` over
    // `reconciled_control_edges`) now finds the officer as a genuine
    // `ControlByOtherMeans` candidate BEFORE any pull is attempted — the
    // internal per-pivot pull this test originally named never fires for
    // this fixture anymore, because `prior_candidates` is no longer empty
    // by the time it would be considered. What remains load-bearing here:
    // the EXTERNAL, generic `pull_smo_on_exhaustion` still correctly
    // declines to pull a SECOND time once the primary walk already
    // produced a candidate — the property the test name is actually about.
    let fund = eid(11);
    let manco = eid(12);
    let officer = PersonId(eid(13).0);
    let mut state = ControlState {
        structure_class: Some(StructureClass::InvestmentFund),
        ..Default::default()
    };
    let mandate = edge(8, EdgeKind::ManagementMandate, manco, fund, 8);
    let officer_edge = edge(9, EdgeKind::OfficerAppointment, EntityId(officer.0), manco, 9);
    state.edges.insert(mandate.id, mandate);
    state.edges.insert(officer_edge.id, officer_edge);
    let natural_persons: BTreeSet<PersonId> = [officer].into_iter().collect();

    let candidates = FundControlStrategy.resolve(&state, fund, &natural_persons, 25.0);
    assert_eq!(
        candidates.len(),
        1,
        "TS.4 Ruling A: FundControlStrategy resolves the officer via the pivot's own \
         control-chain walk: {candidates:#?}"
    );
    assert_eq!(candidates[0].person_id, officer);
    assert_eq!(
        candidates[0].prong,
        Prong::ControlByOtherMeans,
        "R-A: OfficerAppointment is now Traverse-admitted, so the pivot's primary walk finds \
         the officer directly — no pull, no SmoFallback label, ever attempted for this fixture"
    );
    assert_eq!(
        candidates[0].pivot.as_ref().expect("carries its pivot").pivot_entity,
        manco,
        "resolution is still anchored at the pivot (ManCo), never the fund"
    );

    // The generic, subject-anchored pull must NOT double-pull now that
    // resolve() already produced a candidate (§4a: never pushed, and never
    // pulled twice).
    let pulled = pull_smo_on_exhaustion(&state, fund, &natural_persons, &candidates);
    assert!(
        pulled.is_none(),
        "the external pull must decline once FundControlStrategy's own resolve() already \
         surfaced a candidate: {pulled:#?}"
    );
}

// ── §7 gate: containment_never_traversed ──────────────────────────────────────
//
// SUPERSEDED IN PART BY EOP-DD-UBO-BASES-001 §5 R-A (2026-09-08): TS.3 §3
// originally classified BOTH `Employment` and `Containment` `NotControl`.
// R-A reclassifies `Employment` (= `EdgeKind::Employment`, R-A's
// "EmploymentDelegatedAuthority") to `Traverse` — "employment with
// delegated authority is the thing" — leaving `Containment` as the sole
// survivor of this test's original two-kind property (structural scoping,
// not a control basis, per V&S citations). Renamed accordingly; the
// positive covering gate for Employment-as-a-door is
// `employment_is_a_door_not_a_fallback`, immediately below
// `officer_is_an_independent_door_not_gated_on_exhaustion` above (same
// `Traverse` admission, same mechanism, `EdgeKind::OfficerAppointment` and
// `EdgeKind::Employment` are siblings in the `control_admission` match) —
// added in the EOP-DD-UBO-BASES-001 audit closure tranche (2026-09-08);
// this comment previously claimed that gate already existed here when it
// did not (audit item 9).
#[test]
fn containment_never_traversed() {
    // Property: adding the edge, in ANY quantity, changes no determination.
    let company = eid(14);
    let umbrella = eid(15);
    let carol = PersonId(eid(16).0);
    let mut state = ControlState {
        structure_class: Some(StructureClass::PrivateCompany),
        ..Default::default()
    };
    let voting = edge(10, EdgeKind::VotingRights, EntityId(carol.0), company, 10);
    state.edges.insert(voting.id, voting);
    let natural_persons: BTreeSet<PersonId> = [carol].into_iter().collect();

    let baseline = ControlProngStrategy.resolve(&state, company, &natural_persons, 25.0);
    assert_eq!(baseline.len(), 1);
    assert_eq!(baseline[0].person_id, carol);

    // Any quantity of Containment edges — two — must not change the result.
    for i in 0..2u128 {
        let e = edge(30 + i, EdgeKind::Containment, company, umbrella, 30 + i);
        state.edges.insert(e.id, e);
    }
    let with_stray = ControlProngStrategy.resolve(&state, company, &natural_persons, 25.0);
    assert_eq!(
        format!("{baseline:?}"),
        format!("{with_stray:?}"),
        "TS.3 §3: Containment must be inert in ANY quantity (property, not a single-edge check)"
    );
    assert_eq!(control_admission(&EdgeKind::Containment), ControlAdmission::NotControl);
}

// ── §7 gate: unit_issuance_never_confers_control ─────────────────────────────

#[test]
fn unit_issuance_never_confers_control() {
    // EconomicInterest into a fund-typed entity classifies (TS.2 §3
    // `pipe_of`) as Pipe::UnitIssuance — investors route out, economic
    // axis only (K-2), regardless of that classification.
    let fund = eid(17);
    let investor = PersonId(eid(18).0);
    let mut state = ControlState {
        structure_class: Some(StructureClass::InvestmentFund),
        ..Default::default()
    };
    let mut economic = edge(11, EdgeKind::EconomicInterest, EntityId(investor.0), fund, 11);
    economic.percentage = Some(100.0);
    state.edges.insert(economic.id, economic);
    let natural_persons: BTreeSet<PersonId> = [investor].into_iter().collect();

    let control_candidates = ControlProngStrategy.resolve(&state, fund, &natural_persons, 25.0);
    assert!(
        control_candidates.is_empty(),
        "an EconomicInterest/UnitIssuance edge must never confer control: {control_candidates:#?}"
    );
    assert_eq!(control_admission(&EdgeKind::EconomicInterest), ControlAdmission::NotControl);

    // Contrast: it DOES resolve on the ownership axis (investors route
    // out to their own obligation, not silently discarded).
    let ownership_candidates =
        OwnershipProngStrategy.resolve(&state, fund, &natural_persons, 25.0);
    assert_eq!(ownership_candidates.len(), 1);
    assert_eq!(ownership_candidates[0].person_id, investor);
    assert_eq!(ownership_candidates[0].prong, Prong::OwnershipProng);
}

// ── §7 gate: control_is_not_multiplied (K-3) ─────────────────────────────────

#[test]
fn control_is_not_multiplied() {
    // Two-hop CONTROL chain: subject <-VotingRights- mid <-VotingRights- Alice.
    let subject = eid(19);
    let mid = eid(20);
    let alice = PersonId(eid(21).0);
    let mut state = ControlState { structure_class: None, ..Default::default() };
    let e1 = edge(12, EdgeKind::VotingRights, mid, subject, 12);
    let e2 = edge(13, EdgeKind::VotingRights, EntityId(alice.0), mid, 13);
    state.edges.insert(e1.id, e1);
    state.edges.insert(e2.id, e2);
    let natural_persons: BTreeSet<PersonId> = [alice].into_iter().collect();

    let candidates = ControlProngStrategy.resolve(&state, subject, &natural_persons, 25.0);
    assert_eq!(candidates.len(), 1);
    assert_eq!(
        candidates[0].effective_ownership_pct, None,
        "K-3: a dominant-chain control path propagates as control, with NO percentage \
         arithmetic anywhere on it — contrast the equivalent ownership chain below, which \
         DOES multiply"
    );
    assert_eq!(candidates[0].ownership_chain, vec![subject, mid]);

    // Contrast: the equivalent ECONOMIC chain (2 hops, 50% each) DOES
    // multiply — 25%, not 50% and not 100% — proving the "not multiplied"
    // property is specific to control, not a blanket absence of the field.
    let mut econ_state = ControlState { structure_class: None, ..Default::default() };
    let mut e3 = edge(14, EdgeKind::EconomicInterest, mid, subject, 14);
    e3.percentage = Some(50.0);
    let mut e4 = edge(15, EdgeKind::EconomicInterest, EntityId(alice.0), mid, 15);
    e4.percentage = Some(50.0);
    econ_state.edges.insert(e3.id, e3);
    econ_state.edges.insert(e4.id, e4);
    let econ_candidates =
        OwnershipProngStrategy.resolve(&econ_state, subject, &natural_persons, 20.0);
    assert_eq!(econ_candidates.len(), 1);
    assert_eq!(
        econ_candidates[0].effective_ownership_pct,
        Some(25.0),
        "the economic axis DOES multiply (50% * 50% = 25%) — control's None is deliberate, \
         not an oversight"
    );
}

// ── §7 gate: axes_stay_independent_until_determination (K-2) ─────────────────

#[test]
fn axes_stay_independent_until_determination() {
    let subject = eid(22);
    let alice = PersonId(eid(23).0); // economic
    let bob = PersonId(eid(24).0); // control
    let mut state = ControlState { structure_class: None, ..Default::default() };
    let mut econ = edge(16, EdgeKind::EconomicInterest, EntityId(alice.0), subject, 16);
    econ.percentage = Some(50.0);
    let voting = edge(17, EdgeKind::VotingRights, EntityId(bob.0), subject, 17);
    state.edges.insert(econ.id, econ);
    state.edges.insert(voting.id, voting);
    let natural_persons: BTreeSet<PersonId> = [alice, bob].into_iter().collect();

    let ownership = OwnershipProngStrategy.resolve(&state, subject, &natural_persons, 25.0);
    let control = ControlProngStrategy.resolve(&state, subject, &natural_persons, 25.0);

    assert_eq!(ownership.len(), 1, "ownership axis must see only Alice: {ownership:#?}");
    assert_eq!(ownership[0].person_id, alice);
    assert_eq!(ownership[0].prong, Prong::OwnershipProng);

    assert_eq!(control.len(), 1, "control axis must see only Bob: {control:#?}");
    assert_eq!(control[0].person_id, bob);
    assert_eq!(control[0].prong, Prong::ControlByOtherMeans);

    // No cross-contamination: neither axis's candidate set contains the
    // other axis's person.
    assert!(!ownership.iter().any(|c| c.person_id == bob));
    assert!(!control.iter().any(|c| c.person_id == alice));
}

// ── §7 gate: admission_is_exhaustive ─────────────────────────────────────────

#[test]
fn admission_is_exhaustive() {
    // Structural completeness+disjointness proof at the value level (the
    // COMPILE-time proof — a new EdgeKind variant is a compile error in
    // control_admission's match — is enforced by the exhaustive match with
    // no catch-all arm in fold/control.rs itself; this test is the runtime
    // companion, over the full 17-wide wire vocabulary).
    assert_eq!(EDGE_KIND_WIRE_VALUES.len(), 17);

    let all_kinds: Vec<EdgeKind> = vec![
        EdgeKind::EconomicInterest,
        EdgeKind::VotingRights,
        EdgeKind::BoardAppointment,
        EdgeKind::GpStatutory,
        EdgeKind::DesignatedMember,
        EdgeKind::TrustRole(ob_poc_kyc_substrate::TrustRoleKind::Settlor),
        EdgeKind::TrustRole(ob_poc_kyc_substrate::TrustRoleKind::Trustee),
        EdgeKind::TrustRole(ob_poc_kyc_substrate::TrustRoleKind::Protector),
        EdgeKind::TrustRole(ob_poc_kyc_substrate::TrustRoleKind::Beneficiary),
        EdgeKind::Nominee,
        EdgeKind::DominantInfluence,
        EdgeKind::OfficerAppointment,
        EdgeKind::ManagementMandate,
        EdgeKind::MembershipRights,
        EdgeKind::StatutoryAuthority,
        EdgeKind::Employment,
        EdgeKind::Containment,
    ];
    assert_eq!(all_kinds.len(), 17, "TrustRole's 4 sub-kinds bring EdgeKind's 14 arms to 17 wire values");

    let mut traverse = 0;
    let mut stop = 0;
    let mut not_control = 0;
    let mut pierce = 0;
    for kind in &all_kinds {
        match control_admission(kind) {
            ControlAdmission::Traverse => traverse += 1,
            ControlAdmission::Stop => stop += 1,
            ControlAdmission::NotControl => not_control += 1,
            ControlAdmission::Pierce => pierce += 1,
        }
    }
    // EOP-DD-UBO-BASES-001 §5 R-A (2026-09-08): OfficerAppointment/Employment
    // moved NotControl -> Traverse (11 -> 13; 4 -> 2), recorded in
    // docs/eop/EOP-STATE-KYCUBO-D1_State-of-Play.md §7, not by editing this
    // ratified TS.3 gate.
    assert_eq!(traverse, 13, "8 original + ManagementMandate + MembershipRights + \
        OfficerAppointment + Employment = 12 pipe-slots across 10 EdgeKind arms + TrustRole \
        covers 4 wire values -> 13 wire-value classifications");
    assert_eq!(stop, 1, "StatutoryAuthority only");
    assert_eq!(not_control, 2, "EconomicInterest, Containment");
    assert_eq!(pierce, 1, "Nominee only");
    assert_eq!(traverse + stop + not_control + pierce, 17, "every wire value classifies exactly once");
}

// ── §7 gate: determination_runs_at_any_board_state (§2a) ─────────────────────

#[test]
fn determination_runs_at_any_board_state() {
    // A board of purely alleged edges over alleged types still produces a
    // determination, labelled provisional — nothing in the traversal gates
    // on proof.
    let subject = eid(25);
    let alice = PersonId(eid(26).0);
    let mut state = ControlState { structure_class: None, ..Default::default() };
    // Merely Asserted (not Evidenced/Verified).
    let voting = edge(18, EdgeKind::VotingRights, EntityId(alice.0), subject, 18);
    state.edges.insert(voting.id, voting);
    let natural_persons: BTreeSet<PersonId> = [alice].into_iter().collect();

    let mut type_registry = TypeRegistryState::default();
    type_registry
        .types
        .insert(subject, type_record(EntityType::PrivateLimitedCompany, false, 19));

    let candidates = ControlProngStrategy.resolve(&state, subject, &natural_persons, 25.0);
    assert_eq!(candidates.len(), 1, "an unverified/alleged board must still produce a determination");

    let stops = detect_statutory_stops(&state, subject);
    let assurance = compute_assurance(&candidates, &stops, &state, &type_registry);
    assert!(
        assurance.is_provisional(),
        "everything here is alleged (edge Asserted, subject type Alleged) — must be provisional"
    );
}

// ── §7 gate: traversal_decisions_carry_provisionality (§2a) ──────────────────

#[test]
fn traversal_decisions_carry_provisionality() {
    // Two DISTINCT provisionality sources in one determination: an alleged
    // TYPE on a chain intermediate (AllegedType) and a Stop decision
    // resting on an alleged type (AdmissionOnAllegedType) — must not
    // collapse into one flag.
    let swv = eid(27);
    let govt = eid(28);
    let subject_for_chain = eid(29);
    let mid = eid(30);
    let alice = PersonId(eid(31).0);

    // Part A: statutory stop at swv, swv's type alleged.
    let mut state_a = ControlState { structure_class: None, ..Default::default() };
    let stop_edge = edge_with_proof(
        edge(19, EdgeKind::StatutoryAuthority, govt, swv, 20),
        20,
    );
    state_a.edges.insert(stop_edge.id, stop_edge);
    let mut type_registry_a = TypeRegistryState::default();
    type_registry_a
        .types
        .insert(swv, type_record(EntityType::GovernmentDeptStatutoryCorporation, false, 21));
    let stops_a = detect_statutory_stops(&state_a, swv);
    let assurance_a = compute_assurance(&[], &stops_a, &state_a, &type_registry_a);
    assert!(
        assurance_a.reasons.iter().any(|r| matches!(
            r,
            ProvisionalityReason::AdmissionOnAllegedType { entity, .. } if *entity == swv
        )),
        "a Stop decided from an alleged type must carry AdmissionOnAllegedType: {assurance_a:#?}"
    );

    // Part B: an ordinary chain through `mid`, mid's type alleged, edges Verified
    // (isolating the AllegedType signal from AllegedEdge).
    let mut state_b = ControlState { structure_class: None, ..Default::default() };
    let e1 = edge_with_proof(
        edge(21, EdgeKind::VotingRights, mid, subject_for_chain, 22),
        22,
    );
    let e2 = edge_with_proof(
        edge(22, EdgeKind::VotingRights, EntityId(alice.0), mid, 23),
        23,
    );
    state_b.edges.insert(e1.id, e1);
    state_b.edges.insert(e2.id, e2);
    let natural_persons: BTreeSet<PersonId> = [alice].into_iter().collect();
    let candidates_b =
        ControlProngStrategy.resolve(&state_b, subject_for_chain, &natural_persons, 25.0);
    assert_eq!(candidates_b.len(), 1);
    let mut type_registry_b = TypeRegistryState::default();
    type_registry_b
        .types
        .insert(mid, type_record(EntityType::PrivateLimitedCompany, false, 24));
    let assurance_b = compute_assurance(&candidates_b, &[], &state_b, &type_registry_b);
    assert!(
        assurance_b
            .reasons
            .iter()
            .any(|r| matches!(r, ProvisionalityReason::UncitedType { entity } if *entity == mid)),
        "an uncited intermediate-entity type must carry UncitedType (not AdmissionOnAllegedType \
         — no Stop was involved here): {assurance_b:#?}"
    );
    assert!(
        !assurance_b
            .reasons
            .iter()
            .any(|r| matches!(r, ProvisionalityReason::AdmissionOnAllegedType { .. })),
        "AdmissionOnAllegedType is the Stop-specific reason — must not appear for an ordinary \
         Traverse decision: {assurance_b:#?}"
    );

    // The two parts are driven by the SAME underlying fact shape (an
    // alleged type on the entity a decision turned on) but produce
    // DIFFERENT reason sets: part A's Stop decision earns the specific
    // AdmissionOnAllegedType (on top of the general AllegedType — both are
    // legitimately true and NOT merged into one flag); part B's ordinary
    // Traverse decision earns only the general AllegedType. Proven above by
    // the two positive/negative `.any()` checks — never collapsed into one
    // flag, exactly §2a's requirement.
}

// ── §7 gate: statutory_stop_on_alleged_type_is_provisional (§2a narrow case) ─

#[test]
fn statutory_stop_on_alleged_type_is_provisional() {
    let swv = eid(32);
    let govt = eid(33);
    let mut state = ControlState { structure_class: None, ..Default::default() };
    let stop_edge = edge_with_proof(
        edge(23, EdgeKind::StatutoryAuthority, govt, swv, 25),
        25,
    );
    state.edges.insert(stop_edge.id, stop_edge);
    let stops = detect_statutory_stops(&state, swv);
    assert_eq!(stops.len(), 1);

    // Alleged: the stop must be marked provisional, and specifically via
    // AdmissionOnAllegedType.
    let mut alleged_registry = TypeRegistryState::default();
    alleged_registry
        .types
        .insert(swv, type_record(EntityType::GovernmentDeptStatutoryCorporation, false, 26));
    let assurance_alleged = compute_assurance(&[], &stops, &state, &alleged_registry);
    assert!(
        assurance_alleged.reasons.iter().any(|r| matches!(
            r,
            ProvisionalityReason::AdmissionOnAllegedType { entity, .. } if *entity == swv
        )),
        "we stopped believing it is a state body; that belief is not yet proved — the stop \
         must say so: {assurance_alleged:#?}"
    );

    // Proved: the SAME stop, only the type proof flips — the reason must
    // disappear (isolating that the type-proof state, not the stop's mere
    // existence, drives this specific reason).
    let mut proved_registry = TypeRegistryState::default();
    proved_registry
        .types
        .insert(swv, type_record(EntityType::GovernmentDeptStatutoryCorporation, true, 27));
    let assurance_proved = compute_assurance(&[], &stops, &state, &proved_registry);
    assert!(
        !assurance_proved.reasons.iter().any(|r| matches!(
            r,
            ProvisionalityReason::AdmissionOnAllegedType { .. }
        )),
        "once the type is Proved, AdmissionOnAllegedType must NOT fire for this entity: \
         {assurance_proved:#?}"
    );
}
