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

use std::collections::BTreeSet;

use ob_poc_kyc_substrate::{
    compute_assurance, control_admission, detect_statutory_stops, pull_smo_on_exhaustion,
    ControlAdmission, ControlProngStrategy, ControlState, CooperativeMemberStrategy,
    DeterminationStrategy, EdgeId, EdgeKind, EdgeState, EdgeStatus, EntityId, EntityType,
    EntityTypeRecord, EventId, FundControlStrategy, OwnershipProngStrategy, PersonId, Prong,
    ProngCandidate, ProvisionalityReason, StateOwnedStrategy, StructureClass, TypeProofStatus,
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
        evidence_event_id: None,
        originating_event_id: evid(orig_tag),
        trust_revocable: None,
        superseded_by: None,
        pierced_from: None,
    }
}

fn edge_with_status(mut e: EdgeState, status: EdgeStatus) -> EdgeState {
    e.status = status;
    e
}

fn type_record(entity_type: EntityType, proof: TypeProofStatus, orig_tag: u128) -> EntityTypeRecord {
    let proof_event_id = matches!(proof, TypeProofStatus::Proved).then(|| evid(orig_tag));
    EntityTypeRecord { entity_type, proof, originating_event_id: evid(orig_tag), proof_event_id }
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
    // SovereignWealthVehicle <-StatutoryAuthority- GovtDept.
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

#[test]
fn officers_contribute_only_on_exhaustion() {
    let subject = eid(8);
    let alice = PersonId(eid(9).0); // resolves via VotingRights
    let carol = PersonId(eid(10).0); // an officer — must NOT appear when ownership/control resolved

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
    // must change NOTHING: neither the strategy walk (Carol never appears)
    // nor the pull (which never even fires, because prior_candidates is
    // non-empty).
    let officer = edge(6, EdgeKind::OfficerAppointment, EntityId(carol.0), subject, 6);
    resolving.edges.insert(officer.id, officer);
    let candidates_with_officer =
        ControlProngStrategy.resolve(&resolving, subject, &natural_persons, 25.0);
    assert_eq!(
        format!("{candidates_without_officer:?}"),
        format!("{candidates_with_officer:?}"),
        "adding an officer edge to a resolving structure must change NOTHING"
    );
    assert!(
        pull_smo_on_exhaustion(&resolving, subject, &natural_persons, &candidates_with_officer)
            .is_none(),
        "the pull must not fire when prior_candidates is non-empty"
    );

    // (b) Contrast: with NO VotingRights edge (ownership/control genuinely
    // exhausts at `subject`), the SAME officer edge alone DOES get pulled.
    let mut exhausted = ControlState { structure_class: None, ..Default::default() };
    let officer2 = edge(7, EdgeKind::OfficerAppointment, EntityId(carol.0), subject, 7);
    exhausted.edges.insert(officer2.id, officer2);
    let empty_candidates: Vec<ProngCandidate> = vec![];
    let pulled = pull_smo_on_exhaustion(&exhausted, subject, &natural_persons, &empty_candidates);
    assert!(pulled.is_some(), "with nothing else, exhaustion must pull the officer: {pulled:#?}");
    let (pulled_candidates, _) = pulled.unwrap();
    assert_eq!(pulled_candidates.len(), 1);
    assert_eq!(pulled_candidates[0].person_id, carol);
    assert_eq!(pulled_candidates[0].prong, Prong::SmoFallback);
}

// ── §7 gate: smo_fallback_is_recorded_never_silent ───────────────────────────

#[test]
fn smo_fallback_is_recorded_never_silent() {
    // Fund -ManagementMandate-> nothing but ManCo, ManCo has ONLY an
    // OfficerAppointment edge (no natural-person control edge) — ownership
    // and control both exhaust at ManCo (TS.2 Ruling 2a: the mandate
    // holder, not the fund).
    //
    // **Superseded by EOP-DD-KYCUBO-TS.4 §2 Ruling A (2026-08-22), an
    // INTENDED difference, not drift:** at TS.3 landing time,
    // `FundControlStrategy` was still a thin delegate to
    // `ControlProngStrategy`, so its own walk exhausted with ZERO
    // candidates and an EXTERNAL, subject-anchored `pull_smo_on_exhaustion`
    // call was needed to surface the officer. TS.4's `fund_pivot_resolve`
    // makes the per-pivot exhaustion pull INTERNAL to `FundControlStrategy`
    // itself (correctly anchored at the pivot — Ruling 2a's whole point:
    // the naive subject-anchored pull is wrong for co-management, TS.4 §5
    // `co_management_unions_with_per_pivot_paths`), so `resolve()` alone
    // now returns the officer directly. The external, generic
    // `pull_smo_on_exhaustion` — still exercised below — correctly declines
    // to pull a SECOND time once `resolve()` already returned a candidate
    // (§4a: "fires ONLY on exhaustion... never pushed" — `prior_candidates`
    // is no longer empty).
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
        "TS.4 Ruling A: FundControlStrategy now performs its own per-pivot exhaustion \
         pull internally: {candidates:#?}"
    );
    assert_eq!(candidates[0].person_id, officer);
    assert_eq!(candidates[0].prong, Prong::SmoFallback);
    assert_eq!(
        candidates[0].pivot.as_ref().expect("carries its pivot").pivot_entity,
        manco,
        "the internal pull is anchored at the pivot (ManCo), never the fund"
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

// ── §7 gate: employment_and_containment_never_traversed ──────────────────────

#[test]
fn employment_and_containment_never_traversed() {
    // Property: adding EITHER edge, in ANY quantity, changes no determination.
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

    // Any quantity of Employment/Containment edges — three employment, two
    // containment — must not change the result at all.
    for i in 0..3u128 {
        let e = edge(20 + i, EdgeKind::Employment, EntityId(carol.0), company, 20 + i);
        state.edges.insert(e.id, e);
    }
    for i in 0..2u128 {
        let e = edge(30 + i, EdgeKind::Containment, company, umbrella, 30 + i);
        state.edges.insert(e.id, e);
    }
    let with_stray = ControlProngStrategy.resolve(&state, company, &natural_persons, 25.0);
    assert_eq!(
        format!("{baseline:?}"),
        format!("{with_stray:?}"),
        "TS.3 §3: Employment/Containment must be inert in ANY quantity (property, not \
         a single-edge check)"
    );
    assert_eq!(control_admission(&EdgeKind::Employment), ControlAdmission::NotControl);
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
    assert_eq!(traverse, 11, "8 original + ManagementMandate + MembershipRights = 10 pipe-slots \
        across 8 EdgeKind arms + TrustRole covers 4 wire values -> 11 wire-value classifications");
    assert_eq!(stop, 1, "StatutoryAuthority only");
    assert_eq!(not_control, 4, "EconomicInterest, OfficerAppointment, Employment, Containment");
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
        .insert(subject, type_record(EntityType::PrivateLimitedCompany, TypeProofStatus::Alleged, 19));

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
    let stop_edge = edge_with_status(
        edge(19, EdgeKind::StatutoryAuthority, govt, swv, 20),
        EdgeStatus::Verified,
    );
    state_a.edges.insert(stop_edge.id, stop_edge);
    let mut type_registry_a = TypeRegistryState::default();
    type_registry_a
        .types
        .insert(swv, type_record(EntityType::SovereignWealthVehicle, TypeProofStatus::Alleged, 21));
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
    let e1 = edge_with_status(
        edge(21, EdgeKind::VotingRights, mid, subject_for_chain, 22),
        EdgeStatus::Verified,
    );
    let e2 = edge_with_status(
        edge(22, EdgeKind::VotingRights, EntityId(alice.0), mid, 23),
        EdgeStatus::Verified,
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
        .insert(mid, type_record(EntityType::PrivateLimitedCompany, TypeProofStatus::Alleged, 24));
    let assurance_b = compute_assurance(&candidates_b, &[], &state_b, &type_registry_b);
    assert!(
        assurance_b
            .reasons
            .iter()
            .any(|r| matches!(r, ProvisionalityReason::AllegedType { entity } if *entity == mid)),
        "an alleged intermediate-entity type must carry AllegedType (not AdmissionOnAllegedType \
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
    let stop_edge = edge_with_status(
        edge(23, EdgeKind::StatutoryAuthority, govt, swv, 25),
        EdgeStatus::Verified, // isolate: only the TYPE varies below, not the edge
    );
    state.edges.insert(stop_edge.id, stop_edge);
    let stops = detect_statutory_stops(&state, swv);
    assert_eq!(stops.len(), 1);

    // Alleged: the stop must be marked provisional, and specifically via
    // AdmissionOnAllegedType.
    let mut alleged_registry = TypeRegistryState::default();
    alleged_registry
        .types
        .insert(swv, type_record(EntityType::SovereignWealthVehicle, TypeProofStatus::Alleged, 26));
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
        .insert(swv, type_record(EntityType::SovereignWealthVehicle, TypeProofStatus::Proved, 27));
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
