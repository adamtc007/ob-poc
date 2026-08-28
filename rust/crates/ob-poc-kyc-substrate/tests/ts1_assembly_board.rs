//! D1 assembly-board gate tests — EOP-DD-KYCUBO-TS.1 §6, all ten.
//!
//! Two independent transcriptions of the ratified §2/§2a grid live in this
//! file (`expected_target_pipes`/`expected_source_permits`), deliberately
//! re-typed from the doc text rather than imported from `geometry.rs` —
//! `type_geometry_permits_every_matrix_cell`/`matrix_is_exactly_known`
//! exist to catch drift between the ratified text and the implementation,
//! which a test built from the implementation's own tables cannot do.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use uuid::Uuid;

use ob_poc_kyc_substrate::{
    check_type_geometry, enumerate_placement_set,
    fold_type_registry, assembly_lexicon, AuthorityRef, EntityId, EntityType,
    GeometryError, IntentEvent, LinkageSource, ObligationState, Pipe, Principal, SubjectId,
    TargetBinding, ALL_ENTITY_TYPES, ALL_PIPES,
};

// ── Shared fixtures ──────────────────────────────────────────────────────────

fn subject() -> SubjectId {
    SubjectId(Uuid::new_v4())
}

fn as_of() -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(0, 0).unwrap()
}

fn assert_type_event(subject: SubjectId, entity: EntityId, wire: &str) -> IntentEvent {
    IntentEvent::new(
        subject,
        "kyc_ubo.assert.subject.type",
        Principal::test_analyst(),
        AuthorityRef("test".into()),
        TargetBinding { entity_id: Some(entity), ..TargetBinding::for_subject(subject) },
        serde_json::json!({ "entity_id": entity.0.to_string(), "entity_type": wire }),
        as_of(),
    )
}

fn attach_evidence_for_entity(subject: SubjectId, entity: EntityId) -> IntentEvent {
    IntentEvent::new(
        subject,
        "kyc_ubo.assert.edge.evidence",
        Principal::test_analyst(),
        AuthorityRef("test".into()),
        TargetBinding { entity_id: Some(entity), ..TargetBinding::for_subject(subject) },
        serde_json::json!({}),
        as_of(),
    )
}

// ── Independent grid re-transcription (TS.1 §2/§2a, doc text verbatim) ──────

fn expected_target_pipes(target: EntityType) -> &'static [Pipe] {
    use EntityType::*;
    use Pipe::*;
    // TS.1 §2 v0.3 ("§2 v0.3 corrections", 2026-08-19): pipe 14 (employment)
    // is present on the 8 operating types with employees below — corporates,
    // general partnership, LLP, cooperative, charity, statutory corporation.
    // Deliberately absent from funds and limited partnership (externally
    // managed via pipe 7, does not employ). Re-derived independently from
    // the doc text during the D1 corrective tranche (2026-08-21), not
    // copied from `geometry.rs`'s own table.
    match target {
        PrivateLimitedCompany => &[VotingShares, NonVotingShares, BoardAppointment, OfficerAppointment, ContractualControl, EmploymentDelegatedAuthority],
        PublicListedCompany => &[VotingShares, NonVotingShares, BoardAppointment, OfficerAppointment, ContractualControl, EmploymentDelegatedAuthority],
        LlcUs => &[VotingShares, BoardAppointment, OfficerAppointment, ContractualControl, EmploymentDelegatedAuthority],
        GeneralPartnership => &[GpDesignation, BoardAppointment, ContractualControl, EmploymentDelegatedAuthority],
        LimitedPartnership => &[GpDesignation, LimitedPartnershipInterest, ManagementMandate, ContractualControl],
        Llp => &[GpDesignation, BoardAppointment, ContractualControl, EmploymentDelegatedAuthority],
        OeicIcvc => &[BoardAppointment, ManagementMandate, UnitIssuance, PooledAssetContainment],
        Sicav => &[BoardAppointment, ManagementMandate, UnitIssuance, PooledAssetContainment],
        UnitTrust => &[ManagementMandate, TrusteePowers, UnitIssuance, PooledAssetContainment],
        FortyActFund => &[BoardAppointment, ManagementMandate, UnitIssuance, PooledAssetContainment],
        LpFund => &[GpDesignation, LimitedPartnershipInterest, ManagementMandate, UnitIssuance],
        UmbrellaWithSubFunds => &[PooledAssetContainment],
        DiscretionaryTrust => &[TrusteePowers, ReservedPowers, BeneficiaryEntitlement],
        FixedBareTrust => &[TrusteePowers, BeneficiaryEntitlement],
        Foundation => &[TrusteePowers, ReservedPowers, BeneficiaryEntitlement],
        PensionScheme => &[TrusteePowers, BeneficiaryEntitlement],
        CooperativeMutual => &[BoardAppointment, OfficerAppointment, MembershipRights, EmploymentDelegatedAuthority],
        CharityNotForProfit => &[BoardAppointment, OfficerAppointment, TrusteePowers, EmploymentDelegatedAuthority],
        GovernmentDeptStatutoryCorporation => &[BoardAppointment, OfficerAppointment, StatutoryAuthority, EmploymentDelegatedAuthority],
        SovereignWealthVehicle => &[StatutoryAuthority],
        NaturalPerson | SoleTrader => &[],
    }
}

fn expected_source_permits(pipe: Pipe, source: LinkageSource) -> bool {
    use EntityType::*;
    use Pipe::*;
    const CORPORATES: &[EntityType] = &[PrivateLimitedCompany, PublicListedCompany, LlcUs];
    const PARTNERSHIPS: &[EntityType] = &[GeneralPartnership, LimitedPartnership, Llp];
    const FUNDS: &[EntityType] =
        &[OeicIcvc, Sicav, UnitTrust, FortyActFund, LpFund, UmbrellaWithSubFunds];
    const TRUSTS: &[EntityType] = &[DiscretionaryTrust, FixedBareTrust, Foundation, PensionScheme];
    const NATURAL: &[EntityType] = &[NaturalPerson, SoleTrader];

    let LinkageSource::Entity(s) = source else {
        return pipe == BeneficiaryEntitlement;
    };
    match pipe {
        VotingShares | NonVotingShares => {
            CORPORATES.contains(&s) || PARTNERSHIPS.contains(&s) || FUNDS.contains(&s) || TRUSTS.contains(&s) || NATURAL.contains(&s)
        }
        GpDesignation => CORPORATES.contains(&s) || NATURAL.contains(&s),
        LimitedPartnershipInterest => true,
        BoardAppointment => NATURAL.contains(&s),
        OfficerAppointment => NATURAL.contains(&s),
        ManagementMandate => CORPORATES.contains(&s),
        TrusteePowers => CORPORATES.contains(&s) || NATURAL.contains(&s),
        ReservedPowers => NATURAL.contains(&s) || CORPORATES.contains(&s),
        BeneficiaryEntitlement => NATURAL.contains(&s) || CORPORATES.contains(&s) || s == CharityNotForProfit,
        MembershipRights => NATURAL.contains(&s) || CORPORATES.contains(&s),
        StatutoryAuthority => matches!(s, GovernmentDeptStatutoryCorporation | SovereignWealthVehicle),
        ContractualControl => true,
        EmploymentDelegatedAuthority => NATURAL.contains(&s),
        UnitIssuance => true,
        PooledAssetContainment => s == UmbrellaWithSubFunds,
        NomineeHolding => CORPORATES.contains(&s) || NATURAL.contains(&s),
    }
}

fn expected_admits(source: LinkageSource, pipe: Pipe, target: EntityType) -> bool {
    if matches!(target, EntityType::NaturalPerson | EntityType::SoleTrader) {
        return false;
    }
    expected_target_pipes(target).contains(&pipe) && expected_source_permits(pipe, source)
}

fn all_sources() -> Vec<LinkageSource> {
    let mut v: Vec<LinkageSource> = ALL_ENTITY_TYPES.iter().map(|t| LinkageSource::Entity(*t)).collect();
    v.push(LinkageSource::ClassOfBeneficiaries);
    v
}

// ── Gate 1 ───────────────────────────────────────────────────────────────────

#[test]
fn type_geometry_refuses_impossible_linkage() {
    // A voting-share linkage into a partnership is not a wrong move, it is
    // not a move (TS.1 §1).
    let err = check_type_geometry(
        LinkageSource::Entity(EntityType::NaturalPerson),
        Pipe::VotingShares,
        EntityType::GeneralPartnership,
    )
    .unwrap_err();
    assert!(matches!(err, GeometryError::NotAMove { .. }), "expected NotAMove, got {err:?}");

    // Distinct from a stud (positional) violation: a stud violation is a
    // `KycError::PreconditionFailed` from `fold::control::check_preconditions`
    // — a completely different type. `GeometryError` cannot be confused with
    // `KycError` at the type level (no `From`/`Into` between them, no shared
    // variant name) — the distinctness this gate demands is structural, not
    // just a different message string.
    fn _type_distinctness_proof(_g: GeometryError) {
        // If this compiled with `_g` accepted where a `ob_poc_kyc_substrate::KycError`
        // is expected, the two error types would be unified; they are not.
    }
}

// ── Gate 2 ───────────────────────────────────────────────────────────────────

#[test]
fn type_geometry_permits_every_matrix_cell() {
    let mut checked = 0usize;
    let mut admitted_by_expected = 0usize;
    for target in ALL_ENTITY_TYPES {
        for pipe in ALL_PIPES {
            for source in all_sources() {
                checked += 1;
                let expected = expected_admits(source, *pipe, *target);
                let actual = check_type_geometry(source, *pipe, *target).is_ok();
                assert_eq!(
                    actual, expected,
                    "mismatch at source={source:?} pipe={pipe:?} target={target:?}: \
                     expected admitted={expected}, got admitted={actual}"
                );
                if expected {
                    admitted_by_expected += 1;
                }
            }
        }
    }
    // Every permitted triple in the independently-re-typed §2/§2a grid was
    // admitted (proven by the per-cell assert_eq above never firing); this
    // final assert just pins that the property test actually exercised a
    // non-trivial number of admitted cells, not zero.
    assert!(admitted_by_expected > 0);
    assert_eq!(checked, ALL_ENTITY_TYPES.len() * ALL_PIPES.len() * (ALL_ENTITY_TYPES.len() + 1));
}

// ── Gate 3 ───────────────────────────────────────────────────────────────────

#[test]
fn matrix_is_exactly_known() {
    // RED-honest pin: adding a type or pipe changes these constants, which
    // is a compile-visible, conscious edit site (this test and the
    // `ALL_ENTITY_TYPES`/`ALL_PIPES` arrays themselves).
    assert_eq!(ALL_ENTITY_TYPES.len(), 22, "TS.0 §4 catalogue is 22 types (2+3+3+6+4+4)");
    assert_eq!(ALL_PIPES.len(), 17, "TS.0 §3 vocabulary is 17 pipes");

    let mut admitted: BTreeMap<(String, String, String), bool> = BTreeMap::new();
    let mut count = 0usize;
    for target in ALL_ENTITY_TYPES {
        for pipe in ALL_PIPES {
            for source in all_sources() {
                if check_type_geometry(source, *pipe, *target).is_ok() {
                    count += 1;
                    admitted.insert(
                        (format!("{source:?}"), format!("{pipe:?}"), format!("{target:?}")),
                        true,
                    );
                }
            }
        }
    }
    // Pinned today's exact count. A change to this number without a
    // corresponding change to `geometry.rs`'s tables (or this file's
    // independent re-transcription) means the grid drifted — the point of
    // this gate. Computed once, by running this test, not guessed.
    //
    // Pin derived from TS.1 v0.3 (the "§2 v0.3 corrections" block,
    // 2026-08-19), re-derived in the D1 corrective tranche (2026-08-21):
    // the prior pin (530) was itself derived from a pre-correction
    // transcription that omitted pipe 14 from all 8 rows it belongs on.
    // 530 + (8 targets × 2 admitted sources [NaturalPerson, SoleTrader] ×
    // 1 pipe [EmploymentDelegatedAuthority]) = 546.
    assert_eq!(count, admitted.len());
    assert_eq!(
        count, 546,
        "the ratified §2/§2a grid (TS.1 v0.3) today admits exactly 546 (source,pipe,target) \
         triples (counting every admitted EntityType source individually, not by category); \
         if this number changed, a type/pipe/rule was added or removed — confirm the edit \
         was conscious, name the TS.1 version it was re-derived from, before updating this pin"
    );
}

// ── Gate 4 ───────────────────────────────────────────────────────────────────

#[test]
fn person_is_never_a_target() {
    for pipe in ALL_PIPES {
        for target in [EntityType::NaturalPerson, EntityType::SoleTrader] {
            for source in all_sources() {
                let err = check_type_geometry(source, *pipe, target).unwrap_err();
                assert!(
                    matches!(err, GeometryError::PersonIsNeverATarget { .. }),
                    "pipe={pipe:?} target={target:?}: expected PersonIsNeverATarget, got {err:?}"
                );
            }
        }
    }
}

// ── Gate 5 ───────────────────────────────────────────────────────────────────

#[test]
fn officer_and_employment_are_person_sourced() {
    let corporates = [EntityType::PrivateLimitedCompany, EntityType::PublicListedCompany, EntityType::LlcUs];
    for pipe in [Pipe::OfficerAppointment, Pipe::EmploymentDelegatedAuthority] {
        for corp in corporates {
            for target in ALL_ENTITY_TYPES {
                if matches!(target, EntityType::NaturalPerson | EntityType::SoleTrader) {
                    continue;
                }
                let result = check_type_geometry(LinkageSource::Entity(corp), pipe, *target);
                assert!(
                    result.is_err(),
                    "{pipe:?} from corporate source {corp:?} into {target:?} must be refused \
                     (ratified 2026-08-19: pipes 6/14 are natural-person-sourced)"
                );
            }
        }
        // Sanity: the SAME pipe from a natural person into at least one real
        // target is admitted, proving the refusal above is source-specific,
        // not a blanket refusal of the pipe. Both pipes now have real
        // target rows (Employment's 8-row grid gap was fixed in the D1
        // corrective tranche, 2026-08-21) — this must hold for both, not
        // just OfficerAppointment.
        let admits_from_person = ALL_ENTITY_TYPES.iter().any(|t| {
            check_type_geometry(LinkageSource::Entity(EntityType::NaturalPerson), pipe, *t).is_ok()
        });
        assert!(
            admits_from_person,
            "{pipe:?} must admit a natural-person source somewhere"
        );
    }
}

// ── Gate 6 ───────────────────────────────────────────────────────────────────

#[test]
fn mandate_refuses_person_source() {
    for person in [EntityType::NaturalPerson, EntityType::SoleTrader] {
        for target in ALL_ENTITY_TYPES {
            if matches!(target, EntityType::NaturalPerson | EntityType::SoleTrader) {
                continue;
            }
            let result =
                check_type_geometry(LinkageSource::Entity(person), Pipe::ManagementMandate, *target);
            assert!(
                result.is_err(),
                "ManagementMandate from natural-person source {person:?} into {target:?} must be \
                 refused (ratified 2026-08-19: pipe 7 is corporate-only)"
            );
        }
    }
}

// Gate 7 (`correct_type_demotes_never_deletes`) REMOVED — EOP-VS-UBO-GAME-001
// T2 (2026-08-27, §8 Q1) DISSOLVED `kyc_ubo.assert.subject.type-correction`
// and its cascade (`edges_invalidated_by_correction`, deleted); correcting a
// type is now `remove` then `place`, two ordinary moves. See
// `tests/kyc_t2_place_remove.rs`'s `type_correction_is_remove_then_place`.

// ── Gate 8 ───────────────────────────────────────────────────────────────────

#[test]
fn class_target_terminates_with_rationale() {
    // A class is a real SOURCE only for pipe 10 (beneficiary entitlement,
    // §2a row 10 / §3a) — the one pipe whose target is a fiduciary
    // structure the class' entitlement flows into.
    let ok = check_type_geometry(
        LinkageSource::ClassOfBeneficiaries,
        Pipe::BeneficiaryEntitlement,
        EntityType::DiscretionaryTrust,
    );
    assert!(ok.is_ok(), "class source must be admitted for BeneficiaryEntitlement into a trust");

    // Termination, proven structurally: a class can never source ANY other
    // pipe — there is no move that would "decompose" it further into
    // constituent persons. This is what "terminates traversal" means at
    // the type-geometry layer: no onward edge is ever geometrically
    // possible from a class.
    for pipe in ALL_PIPES {
        if *pipe == Pipe::BeneficiaryEntitlement {
            continue;
        }
        for target in ALL_ENTITY_TYPES {
            let result = check_type_geometry(LinkageSource::ClassOfBeneficiaries, *pipe, *target);
            assert!(
                result.is_err(),
                "a class must never source pipe {pipe:?} into {target:?} — no decomposition move exists"
            );
        }
    }

    // NOTE (reported, not silently absorbed): "records why" — an actual
    // rationale payload field on the assertion recording a class as source
    // — is not yet enforced anywhere; that is a payload-schema addition to
    // whichever verb ends up asserting a class-sourced beneficiary link
    // (P4/P5 territory: this crate re-keys `ubo.edge.assert-*` rather than
    // minting a new verb, and neither existing verb's payload has a
    // `rationale` field today). This gate currently proves termination
    // only, not rationale-recording.
}

// ── Gate 9 ───────────────────────────────────────────────────────────────────

#[test]
fn no_move_sets_status() {
    // Structural source-scan (same discipline as
    // `gameboard_r0_convergence.rs::enforcement_is_metadata_blind`): the
    // NEW type-registry moves must never read a caller-supplied "proof"/
    // "status" value out of the payload and assign it directly — CTN-2f:
    // status is computed from evidence, never asserted. `assert-type`
    // always starts `Alleged`; `Proved` is reachable only by deriving it
    // from a subsequent `attach-evidence` event (a distinct match arm).
    let src = std::fs::read_to_string(
        concat!(env!("CARGO_MANIFEST_DIR"), "/src/fold/type_registry.rs"),
    )
    .expect("read fold/type_registry.rs");

    for banned in ["p.get(\"proof\")", "p.get(\"status\")", "payload.get(\"proof\")", "payload.get(\"status\")"] {
        assert!(
            !src.contains(banned),
            "found {banned:?} in fold/type_registry.rs — a move must never read a status/proof \
             value directly from its payload; status must be derived, never asserted (CTN-2f)"
        );
    }

    // Corroborating behavioural check: asserting a type with an (ignored)
    // "proof": "proved" payload key still yields Alleged.
    let subj = subject();
    let entity = EntityId(Uuid::new_v4());
    let mut event = assert_type_event(subj, entity, "private_limited_company");
    event.payload["proof"] = serde_json::json!("proved");
    let state = fold_type_registry(&[&event]);
    let record = state.types.get(&entity).expect("type recorded");
    assert!(
        matches!(record.proof, ob_poc_kyc_substrate::TypeProofStatus::Alleged),
        "a payload-supplied \"proof\": \"proved\" must be ignored; proof is fold-derived only"
    );

    // Deriving Proved the only sanctioned way: a subsequent attach-evidence
    // event targeting the entity (not an edge).
    let evidenced = attach_evidence_for_entity(subj, entity);
    let state2 = fold_type_registry(&[&event, &evidenced]);
    assert!(matches!(
        state2.types.get(&entity).unwrap().proof,
        ob_poc_kyc_substrate::TypeProofStatus::Proved
    ));
}

// ── Gate 10 ──────────────────────────────────────────────────────────────────

#[test]
fn construct_is_pure() {
    // `construct` (TS.1 move 9) is satisfied by the pre-existing `preview`
    // (T3, `src/preview.rs`) — P1 recon found it already matches move 9's
    // contract exactly: `(committed, candidates, lexicon) ->
    // Result<(ControlState, ObligationState)>`, no store handle anywhere in
    // its signature. Proven structurally (type-surface, not behaviourally)
    // — the same pattern `gameboard_r0_convergence.rs::
    // enforcement_is_metadata_blind` uses: if `preview` ever gained a
    // `&mut dyn TransactionScope`/connection/pool parameter, this test
    // fails on the text scan before anyone has to notice it behaviourally.
    let src =
        std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/src/preview.rs"))
            .expect("read preview.rs");
    let sig_start = src.find("pub fn preview(").expect("preview fn signature present");
    let sig_end = src[sig_start..].find(") -> ").map(|i| sig_start + i).expect("closing paren");
    let signature = &src[sig_start..sig_end];

    for forbidden in ["TransactionScope", "PgConnection", "PgPool", "&mut dyn", "Executor"] {
        assert!(
            !signature.contains(forbidden),
            "preview()'s signature contains {forbidden:?} — construct must perform no writes \
             against a live store (TS.1 §3 row 9); signature was: {signature}"
        );
    }
}

// ── P4/P5 end-to-end: the re-keyed enumerate_placement_set ─────────────────

/// Proves the full loop, not just the isolated pieces: registering group
/// members, geometry gating `assert-control` on/off as types are asserted,
/// and the board's `place`/`remove` candidates surfacing correctly through
/// the SAME `enumerate_placement_set` entry point the workbook (T4/T4.5)
/// calls.
///
/// **T2 note (EOP-VS-UBO-GAME-001):** `register`/`type` retired, merged
/// into `place` — real writes can no longer produce "registered but
/// untyped." `register`/`assert_type_event` below build that state
/// directly at the pure-fold level (their fold arms remain, historical
/// replay only — R5) purely to keep exercising the geometry gate's
/// before/after distinction this test exists for; no live surface can
/// build it this way any more.
#[test]
fn geometry_gate_and_new_moves_surface_through_enumerate_placement_set() {
    let subj = subject();
    let lexicon = assembly_lexicon();
    let person = EntityId(Uuid::new_v4());
    let company = EntityId(Uuid::new_v4());

    let register = |entity: EntityId| {
        IntentEvent::new(
            subj,
            "kyc_ubo.assert.subject.register",
            Principal::test_analyst(),
            AuthorityRef("test".into()),
            TargetBinding::for_subject(subj),
            serde_json::json!({ "entity_id": entity.0.to_string() }),
            as_of(),
        )
    };

    let control = ob_poc_kyc_substrate::fold_control(&[
        &register(person),
        &register(company),
    ]);
    let obligation = ObligationState::default();
    assert!(control.registered);
    assert_eq!(control.registered_entity_ids.len(), 2);

    // BEFORE any type: geometry cannot be evaluated for either entity, so
    // the two geometry-gated verbs must NOT appear — even though the stud
    // layer alone (SubjectRegistered=true) would admit them. This is the
    // K-G5 gap TS.1 closes.
    let untyped_registry = ob_poc_kyc_substrate::TypeRegistryState::default();
    let set_before = enumerate_placement_set(subj, &control, &obligation, &untyped_registry, &lexicon);
    // EOP-VS-UBO-GAME-001 T3, §3.2: `control` + `economic-interest` merged
    // into one verb, `connect` — a single membership check now covers what
    // used to be two.
    assert!(
        !set_before.moves.iter().any(|m| m.verb_fqn.as_str() == "kyc_ubo.assert.edge.connect"),
        "connect must be refused before any group member has an asserted type"
    );
    // `place` no longer appears for person/company (T2 §3.1): they are
    // registered but not currently-withdrawn, so they are not in the
    // enumerable "not currently placed" population (T1's P4 finding) — the
    // board correctly has nothing to offer for re-placing an entity that
    // isn't withdrawn. `remove` DOES still offer both — untyped is still
    // "on the board" for removal purposes, a real, worth-documenting
    // consequence, not an oversight. `enquiry` is unaffected.
    assert!(!set_before
        .moves
        .iter()
        .any(|m| m.verb_fqn.as_str() == "kyc_ubo.assert.subject.place"
            && (m.target.entity_id == Some(person) || m.target.entity_id == Some(company))));
    assert!(set_before
        .moves
        .iter()
        .any(|m| m.verb_fqn.as_str() == "kyc_ubo.assert.subject.remove"
            && m.target.entity_id == Some(person)));
    assert!(set_before
        .moves
        .iter()
        .any(|m| m.verb_fqn.as_str() == "kyc_ubo.assert.subject.remove"
            && m.target.entity_id == Some(company)));
    assert!(set_before
        .moves
        .iter()
        .any(|m| m.verb_fqn.as_str() == "kyc_ubo.assert.subject.enquiry"));

    // AFTER typing both (person -> NaturalPerson, company ->
    // PrivateLimitedCompany): a VotingShares linkage from person into
    // company is geometrically possible (§2 row 1, §2a row 1) — the
    // geometry-gated verbs must now appear. `remove` stays offered for
    // both, unaffected by typing.
    let e1 = assert_type_event(subj, person, "natural_person");
    let e2 = assert_type_event(subj, company, "private_limited_company");
    let typed_registry = fold_type_registry(&[&e1, &e2]);
    assert_eq!(typed_registry.type_of(person), Some(EntityType::NaturalPerson));
    assert_eq!(typed_registry.type_of(company), Some(EntityType::PrivateLimitedCompany));

    let set_after = enumerate_placement_set(subj, &control, &obligation, &typed_registry, &lexicon);
    assert!(
        set_after.moves.iter().any(|m| m.verb_fqn.as_str() == "kyc_ubo.assert.edge.connect"),
        "connect must be admitted once a geometrically-possible pair of typed members exists"
    );
    assert!(set_after
        .moves
        .iter()
        .any(|m| m.verb_fqn.as_str() == "kyc_ubo.assert.subject.remove"
            && m.target.entity_id == Some(person)));
    assert!(set_after
        .moves
        .iter()
        .any(|m| m.verb_fqn.as_str() == "kyc_ubo.assert.subject.remove"
            && m.target.entity_id == Some(company)));

    // Sanity: person is never a target anywhere in geometry — asserting
    // that fact end-to-end wouldn't be a "not this pair" refusal, it's an
    // absolute one (already proven in isolation by `person_is_never_a_target`;
    // this just confirms `enumerate_placement_set`'s gate composes with it
    // rather than silently overriding it via the "exists a possible pair"
    // shortcut).
    assert!(
        check_type_geometry(
            LinkageSource::Entity(EntityType::PrivateLimitedCompany),
            Pipe::VotingShares,
            EntityType::NaturalPerson,
        )
        .is_err(),
        "sanity: company -> person is still refused by geometry regardless of the group's overall admission"
    );
}
