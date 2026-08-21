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
//! here assert the INTENDED difference, not sameness (except where §3
//! itself rules "no change": `employment_and_containment_never_traversed`,
//! `unit_issuance_never_confers_control`).
//!
//! Pure — no `DATABASE_URL`. Mirrors `tests/kyc_pack_closure.rs`'s
//! direct-strategy-construction style (`ControlState::default()` + manual
//! edge insertion), not the live-DB op-path style of `kyc_ts2_fund_
//! foundation.rs` — the semantics under test live entirely in
//! `ob-poc-kyc-substrate`, which is deliberately DB-free.

use std::collections::BTreeSet;

use ob_poc_kyc_substrate::{
    CooperativeMemberStrategy, DeterminationStrategy, EdgeId, EdgeKind, EdgeState, EdgeStatus,
    EntityId, EventId, FundControlStrategy, PersonId, Prong, StateOwnedStrategy, StructureClass,
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

// ── Phase 0 — baseline: what determinations produce TODAY (pre-TS.3) ────────
//
// Run with `cargo test --test kyc_ts3_control_admission phase0 -- --nocapture`
// against the tree BEFORE any TS.3 code change. Captured verbatim in the
// tranche receipt as the "before" half of the intended-difference proof.

#[test]
fn phase0_baseline_fund_with_only_management_mandate() {
    // Fund <-ManagementMandate- ManCo <-BoardAppointment- Alice(natural person).
    // FundControlStrategy delegates to ControlProngStrategy's full walk.
    let fund = eid(1);
    let manco = eid(2);
    let alice = PersonId(eid(3).0);
    let mut state = ob_poc_kyc_substrate::ControlState {
        structure_class: Some(StructureClass::InvestmentFund),
        ..Default::default()
    };
    let e1 = edge(1, EdgeKind::ManagementMandate, manco, fund, 1);
    let e2 = edge(2, EdgeKind::BoardAppointment, EntityId(alice.0), manco, 2);
    state.edges.insert(e1.id, e1);
    state.edges.insert(e2.id, e2);
    let natural_persons: BTreeSet<PersonId> = [alice].into_iter().collect();

    let candidates = FundControlStrategy.resolve(&state, fund, &natural_persons, 25.0);
    println!("PHASE0 fund_with_only_management_mandate: {candidates:#?}");
    // BEFORE (pre-TS.3): ManagementMandate excluded from reconciled_control_edges
    // -> adjacency for `fund` is empty -> zero candidates. The fund has NO
    // control path at all — exactly the defect TS.3 §3 names.
    assert!(
        candidates.is_empty(),
        "PRE-TS.3 baseline must be empty (ManagementMandate not yet admitted): {candidates:#?}"
    );
}

#[test]
fn phase0_baseline_cooperative_with_membership_rights() {
    // Coop <-MembershipRights- Bob(natural person). No board/voting edges.
    let coop = eid(4);
    let bob = PersonId(eid(5).0);
    let mut state = ob_poc_kyc_substrate::ControlState {
        structure_class: Some(StructureClass::Cooperative),
        ..Default::default()
    };
    let e1 = edge(3, EdgeKind::MembershipRights, EntityId(bob.0), coop, 3);
    state.edges.insert(e1.id, e1);
    let natural_persons: BTreeSet<PersonId> = [bob].into_iter().collect();

    let candidates = CooperativeMemberStrategy.resolve(&state, coop, &natural_persons, 25.0);
    println!("PHASE0 cooperative_with_membership_rights: {candidates:#?}");
    // BEFORE: CooperativeMemberStrategy's own kind filter is
    // voting_rights|board_appointment|dominant_influence — membership_rights
    // is not in it (and reconciled_control_edges excludes it too) -> empty.
    assert!(
        candidates.is_empty(),
        "PRE-TS.3 baseline must be empty (membership_rights not yet a co-op control axis): \
         {candidates:#?}"
    );
}

#[test]
fn phase0_baseline_state_body_via_statutory_authority() {
    // SovereignWealthVehicle <-StatutoryAuthority- GovtDept. No further edges.
    let swv = eid(6);
    let govt = eid(7);
    let mut state = ob_poc_kyc_substrate::ControlState {
        structure_class: Some(StructureClass::StateOwned),
        ..Default::default()
    };
    let e1 = edge(4, EdgeKind::StatutoryAuthority, govt, swv, 4);
    state.edges.insert(e1.id, e1);
    let natural_persons: BTreeSet<PersonId> = BTreeSet::new();

    let candidates = StateOwnedStrategy.resolve(&state, swv, &natural_persons, 25.0);
    println!("PHASE0 state_body_via_statutory_authority: {candidates:#?}");
    // BEFORE: statutory_authority excluded from reconciled_control_edges ->
    // empty candidates, and NOTHING records why — a silent exclusion, not a
    // recorded stop. This silence is exactly what TS.3 §3/§4a's "Stop must
    // record why" closes.
    assert!(
        candidates.is_empty(),
        "PRE-TS.3 baseline must be empty (statutory_authority already excluded, but silently): \
         {candidates:#?}"
    );
}

#[test]
fn phase0_baseline_employment_and_containment_edges() {
    // A private company with a stray Employment edge and a Containment edge
    // alongside a real VotingRights edge — both should be inert before AND
    // after TS.3 (§3 rules both DO-NOT-ADMIT, unchanged by this tranche).
    let company = eid(8);
    let employer_target = eid(9); // umbrella for Containment's `to`
    let carol = PersonId(eid(10).0);
    let mut state = ob_poc_kyc_substrate::ControlState {
        structure_class: Some(StructureClass::PrivateCompany),
        ..Default::default()
    };
    let voting = edge(5, EdgeKind::VotingRights, EntityId(carol.0), company, 5);
    let employment = edge(6, EdgeKind::Employment, EntityId(carol.0), company, 6);
    let containment = edge(7, EdgeKind::Containment, company, employer_target, 7);
    state.edges.insert(voting.id, voting);
    state.edges.insert(employment.id, employment.clone());
    state.edges.insert(containment.id, containment.clone());
    let natural_persons: BTreeSet<PersonId> = [carol].into_iter().collect();

    let with_stray = ob_poc_kyc_substrate::ControlProngStrategy.resolve(
        &state,
        company,
        &natural_persons,
        25.0,
    );
    println!("PHASE0 employment_and_containment (with stray edges): {with_stray:#?}");

    // Remove the stray edges; VotingRights alone should produce the SAME result.
    state.edges.remove(&employment.id);
    state.edges.remove(&containment.id);
    let without_stray = ob_poc_kyc_substrate::ControlProngStrategy.resolve(
        &state,
        company,
        &natural_persons,
        25.0,
    );
    println!("PHASE0 employment_and_containment (without stray edges): {without_stray:#?}");

    assert_eq!(
        format!("{with_stray:?}"),
        format!("{without_stray:?}"),
        "PRE-TS.3 baseline: adding Employment/Containment edges must already be inert \
         (both excluded today) — this is the ONE case where equivalence, not difference, \
         is the correct assertion, both before and after TS.3 (§3: DO NOT ADMIT, unchanged)"
    );
    assert_eq!(with_stray.len(), 1, "the real VotingRights edge alone must produce one candidate");
    assert_eq!(with_stray[0].person_id, carol);
    assert_eq!(with_stray[0].prong, Prong::ControlByOtherMeans);
}
