//! T4 gate tests — `EOP-DD-UBO-DISPATCH-001` §2/§3/§4/§5 (the 22→8 mapping
//! becomes the ratified 21→8+2 dispatch: 21 real `EntityType`s, 19 of which
//! map to one of 7 live `DeterminationStrategy` names — `nominee_pierce_
//! strategy` is retired as a dispatch TARGET (§2 "Where nominee_pierce_
//! strategy went. Nowhere — deliberately") — and 2 (`NaturalPerson`,
//! `SoleTrader`) are explicitly terminal.
//!
//! Written against the END state: `EntityType` has already had
//! `SovereignWealthVehicle` removed (P2, catalogue 22→21) and
//! `dispatch_for_entity_type`/`DeterminationDispatch` already exist (P3).
//! RED until both land — first a compile error (the symbols don't exist /
//! `ALL_ENTITY_TYPES` is still 22), then a real assertion failure once P3
//! lands without P2, same RED-first discipline as `kyc_t3_connect_
//! disconnect.rs`.
//!
//! Pure — no `DATABASE_URL`. Mirrors `kyc_ts3_control_admission.rs`'s
//! direct-strategy-construction style: the semantics under test live
//! entirely in `ob-poc-kyc-substrate`.
//!
//! - `every_entity_type_has_a_ruling` — every one of the 21 `EntityType`s
//!   dispatches to `Strategy(_)` or `NotADeterminationSubject`; none is
//!   unreachable from `dispatch_for_entity_type`.
//! - `terminal_types_are_explicit` — `NaturalPerson`/`SoleTrader` are
//!   `NotADeterminationSubject`, a real, distinct variant — not an omission.
//! - `unmapped_type_refuses_freeze_by_name` — the fail-closed property at
//!   the precondition-check chokepoint: a terminal-typed subject refuses,
//!   naming the type; an untyped subject refuses, saying so.
//! - `dispatch_is_exhaustive` — every `ALL_ENTITY_TYPES` member round-trips
//!   through `dispatch_for_entity_type` without panicking (the REAL
//!   exhaustiveness guarantee — §4 D3, "a new entity type is a compile
//!   error" — is enforced by rustc's non-catch-all match, not by this test;
//!   this test documents the guarantee's visible effect).
//! - `mapping_matches_the_ratified_table` — a hand-authored, independent
//!   copy of §2's table, diffed against the real function for every type
//!   (§4 D4: the mapping is data with a test, not trust in the function's
//!   own arms).
//! - Five behavioural gates for the types whose strategy actually CHANGES
//!   relative to today's 11-`StructureClass`/8-arm mapping — `LlcUs`,
//!   `LpFund`, `UnitTrust`, `UmbrellaWithSubFunds`, `CharityNotForProfit`
//!   (identified in P0: 3 are explicitly "RULED 2026-08-27" rows in §2,
//!   2 more are explicit "tension" callouts in the table text itself).
//!   Each builds a fixture, dispatches, resolves via the REAL selected
//!   strategy, and — where a plausible wrong strategy exists — shows that
//!   strategy would have produced a DIFFERENT (or empty) result, so the
//!   choice is shown to matter, not just recorded.

use std::collections::{BTreeMap, BTreeSet};

use ob_poc_kyc_substrate::{
    check_preconditions, ControlProngStrategy, ControlState, DeterminationDispatch,
    DeterminationStrategy, EdgeId, EdgeKind, EdgeState, EdgeStatus, EntityId, EntityType,
    EntityTypeRecord, EventId, FundControlStrategy, LexiconEntry, PersonId, ProofKind,
    ProofRecord, Prong, TargetBinding, TrustRoleKind, TrustRoleStrategy, TypeRegistryState,
    ALL_ENTITY_TYPES,
};

// ── Fixture builders (mirrors kyc_ts3_control_admission.rs) ────────────────

fn eid(tag: u128) -> EntityId {
    EntityId(uuid::Uuid::from_u128(0xF4_D15_0000_0000_0000_0000_0000 | tag))
}
fn evid(tag: u128) -> EventId {
    EventId(uuid::Uuid::from_u128(0xF4_D15_0000_0000_0000_0000_E000 | tag))
}

fn edge(id_tag: u128, kind: EdgeKind, from: EntityId, to: EntityId, orig_tag: u128) -> EdgeState {
    EdgeState {
        id: EdgeId(uuid::Uuid::from_u128(0xF4_D15_0000_0000_0000_0000_D000 | id_tag)),
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

fn type_record(entity_type: EntityType, orig_tag: u128) -> EntityTypeRecord {
    let citing = evid(orig_tag);
    let mut proofs = BTreeMap::new();
    proofs.insert(
        citing,
        ProofRecord {
            kind: ProofKind::FiledDocument,
            source: "test fixture".to_string(),
            date: "2026-08-28".to_string(),
            event_id: citing,
        },
    );
    EntityTypeRecord { entity_type, originating_event_id: citing, proofs }
}

fn registry_with(subject: EntityId, t: EntityType) -> TypeRegistryState {
    let mut tr = TypeRegistryState::default();
    tr.types.insert(subject, type_record(t, 999));
    tr
}

// ── every_entity_type_has_a_ruling ──────────────────────────────────────────

#[test]
fn every_entity_type_has_a_ruling() {
    assert_eq!(
        ALL_ENTITY_TYPES.len(),
        21,
        "EOP-DD-UBO-DISPATCH-001 §3a: catalogue drops to 21 (SovereignWealthVehicle removed, P2)"
    );
    for t in ALL_ENTITY_TYPES {
        match ob_poc_kyc_substrate::dispatch_for_entity_type(t) {
            DeterminationDispatch::Strategy(name) => {
                assert!(!name.is_empty(), "{t:?} dispatched to an empty strategy name");
            }
            DeterminationDispatch::NotADeterminationSubject => {}
        }
    }
}

// ── terminal_types_are_explicit ─────────────────────────────────────────────

#[test]
fn terminal_types_are_explicit() {
    for t in [EntityType::NaturalPerson, EntityType::SoleTrader] {
        assert_eq!(
            ob_poc_kyc_substrate::dispatch_for_entity_type(&t),
            DeterminationDispatch::NotADeterminationSubject,
            "{t:?} must be an explicit terminal arm (§4 D2), not an omission"
        );
    }
    // Distinguishable from a real strategy arm — not the same variant shape.
    assert_ne!(
        ob_poc_kyc_substrate::dispatch_for_entity_type(&EntityType::NaturalPerson),
        ob_poc_kyc_substrate::dispatch_for_entity_type(&EntityType::PrivateLimitedCompany),
    );
}

// ── unmapped_type_refuses_freeze_by_name ────────────────────────────────────

fn freeze_entry() -> LexiconEntry {
    ob_poc_kyc_substrate::assembly_lexicon()
        .get("kyc_ubo.decide.determination.freeze")
        .expect("freeze in lexicon")
        .clone()
}

fn probe_freeze_event(subject: ob_poc_kyc_substrate::SubjectId) -> ob_poc_kyc_substrate::IntentEvent {
    ob_poc_kyc_substrate::IntentEvent::new(
        subject,
        "kyc_ubo.decide.determination.freeze",
        ob_poc_kyc_substrate::Principal::test_analyst(),
        ob_poc_kyc_substrate::AuthorityRef("t4-probe".into()),
        TargetBinding::for_subject(subject),
        serde_json::Value::Null,
        chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0).unwrap(),
    )
}

#[test]
fn unmapped_type_refuses_freeze_by_name() {
    let entry = freeze_entry();
    let subject = ob_poc_kyc_substrate::SubjectId(uuid::Uuid::new_v4());
    let subject_entity = EntityId(subject.0);
    let control = ControlState::default();
    let event = probe_freeze_event(subject);

    // Terminal type: refuses, naming the type.
    let terminal_registry = registry_with(subject_entity, EntityType::NaturalPerson);
    let err = check_preconditions(&entry, &control, &terminal_registry, &event)
        .expect_err("a NaturalPerson subject must refuse freeze");
    let msg = err.to_string();
    assert!(
        msg.contains("NaturalPerson"),
        "refusal must name the type — got: {msg}"
    );

    // No type known at all: refuses, saying so.
    let empty_registry = TypeRegistryState::default();
    let err = check_preconditions(&entry, &control, &empty_registry, &event)
        .expect_err("a subject with no known entity type must refuse freeze");
    assert!(
        !err.to_string().is_empty(),
        "refusal for an untyped subject must carry a reason"
    );

    // A supported type: admits.
    let ok_registry = registry_with(subject_entity, EntityType::PrivateLimitedCompany);
    check_preconditions(&entry, &control, &ok_registry, &event)
        .expect("a PrivateLimitedCompany subject must be admitted to freeze");
}

// ── dispatch_is_exhaustive ───────────────────────────────────────────────────

#[test]
fn dispatch_is_exhaustive() {
    // The real guarantee (§4 D3) is rustc's non-catch-all match in
    // `dispatch_for_entity_type` — a new `EntityType` variant fails to
    // compile there, not here. This test documents the visible effect:
    // every member of the catalogue resolves without panicking.
    let results: Vec<DeterminationDispatch> = ALL_ENTITY_TYPES
        .iter()
        .map(ob_poc_kyc_substrate::dispatch_for_entity_type)
        .collect();
    assert_eq!(results.len(), ALL_ENTITY_TYPES.len());
}

// ── mapping_matches_the_ratified_table ──────────────────────────────────────

#[test]
fn mapping_matches_the_ratified_table() {
    use EntityType::*;
    // EOP-DD-UBO-DISPATCH-001 §2, verbatim — a second, independent copy so
    // a code change that drifts from the ratified table is caught (§4 D4).
    let ratified: BTreeMap<EntityType, DeterminationDispatch> = BTreeMap::from([
        (NaturalPerson, DeterminationDispatch::NotADeterminationSubject),
        (SoleTrader, DeterminationDispatch::NotADeterminationSubject),
        (PrivateLimitedCompany, DeterminationDispatch::Strategy("ownership_prong_strategy")),
        (PublicListedCompany, DeterminationDispatch::Strategy("ownership_prong_strategy")),
        (LlcUs, DeterminationDispatch::Strategy("control_prong_strategy")),
        (GeneralPartnership, DeterminationDispatch::Strategy("control_prong_strategy")),
        (LimitedPartnership, DeterminationDispatch::Strategy("control_prong_strategy")),
        (Llp, DeterminationDispatch::Strategy("control_prong_strategy")),
        (OeicIcvc, DeterminationDispatch::Strategy("fund_control_strategy")),
        (Sicav, DeterminationDispatch::Strategy("fund_control_strategy")),
        (UnitTrust, DeterminationDispatch::Strategy("fund_control_strategy")),
        (FortyActFund, DeterminationDispatch::Strategy("fund_control_strategy")),
        (LpFund, DeterminationDispatch::Strategy("fund_control_strategy")),
        (UmbrellaWithSubFunds, DeterminationDispatch::Strategy("fund_control_strategy")),
        (DiscretionaryTrust, DeterminationDispatch::Strategy("trust_role_strategy")),
        (FixedBareTrust, DeterminationDispatch::Strategy("trust_role_strategy")),
        (Foundation, DeterminationDispatch::Strategy("foundation_council_strategy")),
        (PensionScheme, DeterminationDispatch::Strategy("trust_role_strategy")),
        (CooperativeMutual, DeterminationDispatch::Strategy("cooperative_member_strategy")),
        (CharityNotForProfit, DeterminationDispatch::Strategy("trust_role_strategy")),
        (
            GovernmentDeptStatutoryCorporation,
            DeterminationDispatch::Strategy("state_owned_strategy"),
        ),
    ]);
    assert_eq!(ratified.len(), 21, "the ratified table itself must name all 21 types");

    let mut mismatches = Vec::new();
    for t in ALL_ENTITY_TYPES {
        let expected = ratified.get(t).unwrap_or_else(|| {
            panic!("{t:?} is in ALL_ENTITY_TYPES but not in this test's ratified-table copy")
        });
        let actual = ob_poc_kyc_substrate::dispatch_for_entity_type(t);
        if actual != *expected {
            mismatches.push(format!("{t:?}: expected {expected:?}, got {actual:?}"));
        }
    }
    assert!(
        mismatches.is_empty(),
        "dispatch_for_entity_type drifted from EOP-DD-UBO-DISPATCH-001 §2: {mismatches:#?}"
    );
}

// ── Behavioural gates: the 5 types whose strategy CHANGES ──────────────────

fn strategy_for(name: &str) -> &'static dyn DeterminationStrategy {
    match name {
        "ownership_prong_strategy" => &ob_poc_kyc_substrate::OwnershipProngStrategy,
        "control_prong_strategy" => &ControlProngStrategy,
        "trust_role_strategy" => &TrustRoleStrategy,
        "fund_control_strategy" => &FundControlStrategy,
        "foundation_council_strategy" => &ob_poc_kyc_substrate::FoundationCouncilStrategy,
        "state_owned_strategy" => &ob_poc_kyc_substrate::StateOwnedStrategy,
        "cooperative_member_strategy" => &ob_poc_kyc_substrate::CooperativeMemberStrategy,
        other => panic!("test helper strategy_for: unknown strategy name {other}"),
    }
}

/// §2 Q1, RULED: `LlcUs` -> `control_prong_strategy`, not `ownership_prong_strategy`
/// (the bucket an analyst would have reached for under the old 11-class
/// system, absent an LLC-specific class).
#[test]
fn llc_us_dispatches_to_control_prong_not_ownership_prong() {
    assert_eq!(
        ob_poc_kyc_substrate::dispatch_for_entity_type(&EntityType::LlcUs),
        DeterminationDispatch::Strategy("control_prong_strategy"),
    );

    let llc = eid(1);
    let manager = PersonId(eid(2).0);
    let mut state = ControlState::default();
    // Managed by a designated manager exercising dominant influence — control
    // by designation (§2 row 5's reasoning), not by equity.
    // `OfficerAppointment` is deliberately `NotControl` (ruled 2026-08-21,
    // `control_admission` doc comment — "may not be needed or relevant" to
    // the control walk), so the fixture uses `DominantInfluence`, which IS
    // `Traverse`-admitted. No `EconomicInterest` edge exists at all, so
    // ownership_prong_strategy (reads only economic edges) MUST find nothing.
    let e = edge(1, EdgeKind::DominantInfluence, EntityId(manager.0), llc, 1);
    state.edges.insert(e.id, e);
    let natural_persons: BTreeSet<PersonId> = [manager].into_iter().collect();

    let control_candidates = strategy_for("control_prong_strategy").resolve(&state, llc, &natural_persons, 25.0);
    assert_eq!(control_candidates.len(), 1, "control_prong must resolve the designated manager: {control_candidates:#?}");
    assert_eq!(control_candidates[0].person_id, manager);
    assert_eq!(control_candidates[0].prong, Prong::ControlByOtherMeans);

    let ownership_candidates =
        strategy_for("ownership_prong_strategy").resolve(&state, llc, &natural_persons, 25.0);
    assert!(
        ownership_candidates.is_empty(),
        "ownership_prong must find nothing on a pure control-designation LLC — proves the choice matters: {ownership_candidates:#?}"
    );
}

/// §2 row 13, and the table's own "distinguished from 7" note: `LpFund`
/// splits off the old singular "lp_fund" `StructureClass` bucket (which
/// used to give `control_prong_strategy` uniformly) into plain
/// `LimitedPartnership` (still `control_prong_strategy`) and `LpFund`
/// (now `fund_control_strategy` — the mandate pivot).
#[test]
fn lp_fund_dispatches_to_fund_control_not_control_prong() {
    assert_eq!(
        ob_poc_kyc_substrate::dispatch_for_entity_type(&EntityType::LpFund),
        DeterminationDispatch::Strategy("fund_control_strategy"),
    );
    assert_eq!(
        ob_poc_kyc_substrate::dispatch_for_entity_type(&EntityType::LimitedPartnership),
        DeterminationDispatch::Strategy("control_prong_strategy"),
        "the plain (non-fund) LP stays on control_prong — only the fund-shaped sibling moves"
    );

    let fund = eid(3);
    let manco = eid(4);
    let alice = PersonId(eid(5).0);
    let bob = PersonId(eid(6).0);
    let mut state = ControlState::default();
    // The governing mandate: ManCo manages the fund, Alice controls the ManCo.
    let e1 = edge(3, EdgeKind::ManagementMandate, manco, fund, 3);
    let e2 = edge(4, EdgeKind::BoardAppointment, EntityId(alice.0), manco, 4);
    // A limited partner with a stray, directly-asserted voting-rights edge on
    // the fund itself (e.g. a consent right over a narrow reserved matter) —
    // `VotingRights` IS `Traverse`-admitted control, so the GENERIC control
    // walk picks Bob up as a candidate. `fund_control_strategy`'s pivot
    // discipline (`governing_mandate_edges_into` — ManagementMandate/
    // GpStatutory ONLY) deliberately does not: once a governing mandate
    // exists, resolution runs through it exclusively, not through every
    // control-kind edge asserted directly on the fund.
    let e3 = edge(5, EdgeKind::VotingRights, EntityId(bob.0), fund, 5);
    state.edges.insert(e1.id, e1);
    state.edges.insert(e2.id, e2);
    state.edges.insert(e3.id, e3);
    let natural_persons: BTreeSet<PersonId> = [alice, bob].into_iter().collect();

    let fund_candidates = strategy_for("fund_control_strategy").resolve(&state, fund, &natural_persons, 25.0);
    assert_eq!(fund_candidates.len(), 1, "fund_control must pivot through the ManCo mandate only, excluding Bob's stray voting-rights edge: {fund_candidates:#?}");
    assert_eq!(fund_candidates[0].person_id, alice);

    // control_prong walks EVERY control-kind edge into the fund, not just
    // governing mandates — it picks up both Alice (via the mandate chain)
    // AND Bob (via the stray voting-rights edge fund_control deliberately
    // excludes): proves fund_control's pivot discipline is load-bearing.
    let control_candidates = strategy_for("control_prong_strategy").resolve(&state, fund, &natural_persons, 25.0);
    let control_ids: BTreeSet<PersonId> = control_candidates.iter().map(|c| c.person_id).collect();
    assert_eq!(
        control_ids,
        [alice, bob].into_iter().collect(),
        "control_prong (no pivot discipline) must find BOTH Alice and Bob: {control_candidates:#?}"
    );
}

/// §2 row 11's own tension note: "this is a trust-shaped vehicle taking the
/// fund strategy, correctly, because its directing mind is a mandate
/// holder" — the superficial trustee framing must NOT win.
#[test]
fn unit_trust_dispatches_to_fund_control_not_trust_role() {
    assert_eq!(
        ob_poc_kyc_substrate::dispatch_for_entity_type(&EntityType::UnitTrust),
        DeterminationDispatch::Strategy("fund_control_strategy"),
    );

    let trust = eid(6);
    let manco = eid(7);
    let trustee = eid(8); // holds legal title, does NOT direct (per §2 row 11)
    let bob = PersonId(eid(9).0);
    let mut state = ControlState::default();
    let e1 = edge(5, EdgeKind::ManagementMandate, manco, trust, 5);
    let e2 = edge(6, EdgeKind::DominantInfluence, EntityId(bob.0), manco, 6);
    // The trustee holds legal title (TrusteePowers into the trust) but is
    // NOT the directing mind — trust_role_strategy would resolve THIS
    // person; fund_control_strategy must not.
    let e3 = edge(7, EdgeKind::TrustRole(TrustRoleKind::Trustee), trustee, trust, 7);
    state.edges.insert(e1.id, e1);
    state.edges.insert(e2.id, e2);
    state.edges.insert(e3.id, e3);
    let natural_persons: BTreeSet<PersonId> = [bob].into_iter().collect();

    let fund_candidates = strategy_for("fund_control_strategy").resolve(&state, trust, &natural_persons, 25.0);
    assert_eq!(fund_candidates.len(), 1, "fund_control must pivot to the ManCo's directing mind: {fund_candidates:#?}");
    assert_eq!(fund_candidates[0].person_id, bob, "must resolve the mandate holder, not the trustee");

    let trust_role_candidates =
        strategy_for("trust_role_strategy").resolve(&state, trust, &natural_persons, 25.0);
    assert!(
        trust_role_candidates.is_empty(),
        "trust_role_strategy resolves only real PersonId trustees/settlors/beneficiaries — the \
         trustee here (`trustee`) is not itself a natural person in `natural_persons`, so it must \
         find nothing: proves the superficial trust framing does not silently win: {trust_role_candidates:#?}"
    );
}

/// §2 Q2, RULED: `UmbrellaWithSubFunds` is now a determination subject at
/// all (previously unmapped under `StructureClass` — no "umbrella" wire
/// value existed), and gets `fund_control_strategy`.
#[test]
fn umbrella_with_sub_funds_is_now_a_determination_subject() {
    assert_eq!(
        ob_poc_kyc_substrate::dispatch_for_entity_type(&EntityType::UmbrellaWithSubFunds),
        DeterminationDispatch::Strategy("fund_control_strategy"),
    );

    let umbrella = eid(10);
    let manco = eid(11);
    let carol = PersonId(eid(12).0);
    let mut state = ControlState::default();
    let e1 = edge(8, EdgeKind::ManagementMandate, manco, umbrella, 8);
    let e2 = edge(9, EdgeKind::BoardAppointment, EntityId(carol.0), manco, 9);
    state.edges.insert(e1.id, e1);
    state.edges.insert(e2.id, e2);
    let natural_persons: BTreeSet<PersonId> = [carol].into_iter().collect();

    let candidates = strategy_for("fund_control_strategy").resolve(&state, umbrella, &natural_persons, 25.0);
    assert_eq!(
        candidates.len(),
        1,
        "an umbrella directed by a ManCo must now resolve — it was unmapped before this tranche: {candidates:#?}"
    );
    assert_eq!(candidates[0].person_id, carol);
}

/// §2 Q3, RULED: `CharityNotForProfit` -> `trust_role_strategy`, not
/// `foundation_council_strategy` (the bucket an analyst might reach for by
/// analogy, given a charity's council-shaped governance) — the DISPATCH
/// TABLE assignment still matters (it is what `freeze` records as the
/// basis, K-1/K-35 auditability), even though both strategies now RESOLVE
/// the same candidate for this edge shape.
///
/// EOP-DD-UBO-DISPATCH-001 Foundation ruling (T4-close, 2026-08-28):
/// `foundation_council_strategy`'s edge-kind filter was corrected from
/// `BoardAppointment`/`DominantInfluence` (geometrically impossible onto a
/// real `Foundation`) to `TrustRole`-kind edges — the same filter
/// `trust_role_strategy` uses. The two strategies are now behaviorally
/// identical (§5q "merge candidate" observation, deferred to the next
/// strategy review, not implemented here) — this test's second half used
/// to prove "the choice matters" by showing `foundation_council_strategy`
/// couldn't see a `TrustRole` edge at all; that property no longer holds,
/// so the test is rewritten to prove what's still true: the DISPATCH still
/// resolves to `trust_role_strategy` by name (the recorded basis on a real
/// freeze), even though `foundation_council_strategy` would now reach the
/// identical answer if it were (wrongly) dispatched to instead.
#[test]
fn charity_dispatches_to_trust_role_not_foundation_council() {
    assert_eq!(
        ob_poc_kyc_substrate::dispatch_for_entity_type(&EntityType::CharityNotForProfit),
        DeterminationDispatch::Strategy("trust_role_strategy"),
    );

    let charity = eid(13);
    let trustee = PersonId(eid(14).0);
    let mut state = ControlState::default();
    let e = edge(10, EdgeKind::TrustRole(TrustRoleKind::Trustee), EntityId(trustee.0), charity, 10);
    state.edges.insert(e.id, e);
    let natural_persons: BTreeSet<PersonId> = [trustee].into_iter().collect();

    let trust_role_candidates =
        strategy_for("trust_role_strategy").resolve(&state, charity, &natural_persons, 25.0);
    assert_eq!(trust_role_candidates.len(), 1, "trust_role must resolve the charity's trustee: {trust_role_candidates:#?}");
    assert_eq!(trust_role_candidates[0].person_id, trustee);

    // foundation_council_strategy now resolves the SAME candidate for this
    // edge shape (post-Foundation-ruling correction) — no longer proof
    // that "the choice matters" behaviorally, only that the DISPATCH TABLE
    // entry (checked above) is what determines the recorded basis.
    let foundation_candidates =
        strategy_for("foundation_council_strategy").resolve(&state, charity, &natural_persons, 25.0);
    assert_eq!(
        format!("{foundation_candidates:?}"),
        format!("{trust_role_candidates:?}"),
        "foundation_council_strategy and trust_role_strategy are now behaviorally \
         identical (same TrustRole-kind filter, same per-role admission) — a real \
         fact of the Foundation ruling, not a bug; got: {foundation_candidates:#?}"
    );
}
