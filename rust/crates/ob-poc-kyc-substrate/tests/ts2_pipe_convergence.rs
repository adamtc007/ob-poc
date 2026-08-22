//! D1 Part B — RED-first gates for pipe vocabulary convergence
//! (EOP-DD-KYCUBO-TS.2 §6). Six of the seven gates are pure/crate-level;
//! the seventh (`unparseable_wire_value_is_not_dominant_influence`) needs a
//! real DB dispatch to prove the live append path, and lives in
//! `rust/tests/kyc_ts2_convergence_live.rs` instead.

use ob_poc_kyc_substrate::{
    check_type_geometry, fold_control, fold_type_registry, pipe_of, AuthorityRef, EdgeKind,
    EdgeState, EntityId, EntityType, IntentEvent, LinkageSource, Pipe, Principal, SubjectId,
    TargetBinding, TrustRoleKind, ALL_PIPES,
};

fn subject() -> SubjectId {
    SubjectId(uuid::Uuid::new_v4())
}

/// Every `EdgeKind` variant, including all four `TrustRole` sub-kinds — the
/// stored-vocabulary analogue of `geometry::ALL_PIPES` (no such const exists
/// on `EdgeKind` itself; enumerated here, independently, the same
/// discipline `ts1_assembly_board.rs` uses for its own re-transcriptions).
fn all_edge_kinds() -> Vec<EdgeKind> {
    vec![
        EdgeKind::EconomicInterest,
        EdgeKind::VotingRights,
        EdgeKind::BoardAppointment,
        EdgeKind::GpStatutory,
        EdgeKind::DesignatedMember,
        EdgeKind::TrustRole(TrustRoleKind::Settlor),
        EdgeKind::TrustRole(TrustRoleKind::Trustee),
        EdgeKind::TrustRole(TrustRoleKind::Protector),
        EdgeKind::TrustRole(TrustRoleKind::Beneficiary),
        EdgeKind::Nominee,
        EdgeKind::DominantInfluence,
        EdgeKind::OfficerAppointment,
        EdgeKind::ManagementMandate,
        EdgeKind::MembershipRights,
        EdgeKind::StatutoryAuthority,
        EdgeKind::Employment,
        EdgeKind::Containment,
    ]
}

// ── pipe_of_is_total ────────────────────────────────────────────────────────

#[test]
fn pipe_of_is_total() {
    // The compiler already proves this (pipe_of's match has no wildcard
    // arm) — this test documents and exercises it at every (EdgeKind,
    // Option<EntityType>) combination, including every known target type
    // and the unknown (`None`) case, asserting no panic and a definite
    // `Pipe` every time (TS.2 §6).
    for kind in all_edge_kinds() {
        let _ = pipe_of(&kind, None);
        for target in ob_poc_kyc_substrate::ALL_ENTITY_TYPES {
            let _ = pipe_of(&kind, Some(*target));
        }
    }
}

// ── pipe_mapping_is_exactly_known ──────────────────────────────────────────

#[test]
fn pipe_mapping_is_exactly_known() {
    // RED-honest pin of TS.2 §3's table, independently re-transcribed (not
    // derived from `pipe_of`'s own source) so drift is caught.
    let direct: &[(EdgeKind, Pipe)] = &[
        (EdgeKind::VotingRights, Pipe::VotingShares),
        (EdgeKind::BoardAppointment, Pipe::BoardAppointment),
        (EdgeKind::GpStatutory, Pipe::GpDesignation),
        (EdgeKind::DesignatedMember, Pipe::GpDesignation), // §7 Q2: shared
        (EdgeKind::TrustRole(TrustRoleKind::Trustee), Pipe::TrusteePowers),
        (EdgeKind::TrustRole(TrustRoleKind::Settlor), Pipe::ReservedPowers),
        (EdgeKind::TrustRole(TrustRoleKind::Protector), Pipe::ReservedPowers),
        (EdgeKind::TrustRole(TrustRoleKind::Beneficiary), Pipe::BeneficiaryEntitlement),
        (EdgeKind::Nominee, Pipe::NomineeHolding),
        (EdgeKind::DominantInfluence, Pipe::ContractualControl),
        (EdgeKind::OfficerAppointment, Pipe::OfficerAppointment),
        (EdgeKind::ManagementMandate, Pipe::ManagementMandate),
        (EdgeKind::MembershipRights, Pipe::MembershipRights),
        (EdgeKind::StatutoryAuthority, Pipe::StatutoryAuthority),
        (EdgeKind::Employment, Pipe::EmploymentDelegatedAuthority),
        (EdgeKind::Containment, Pipe::PooledAssetContainment),
    ];
    for (kind, expected_pipe) in direct {
        let classification = pipe_of(kind, None);
        assert_eq!(
            classification.pipe, Some(*expected_pipe),
            "{kind:?} must classify to {expected_pipe:?} regardless of target type"
        );
        assert!(!classification.provisional, "{kind:?} classification must be certain");
    }
    assert_eq!(direct.len(), 16, "16 direct EdgeKind variants (17 total minus EconomicInterest)");
}

// ── economic_interest_classifies_by_target_type ────────────────────────────

#[test]
fn economic_interest_classifies_by_target_type() {
    use EntityType::*;
    let corporate_cases = [PrivateLimitedCompany, PublicListedCompany, LlcUs];
    for t in corporate_cases {
        let c = pipe_of(&EdgeKind::EconomicInterest, Some(t));
        assert_eq!(c.pipe, Some(Pipe::NonVotingShares));
        assert!(!c.provisional);
    }
    for t in [LimitedPartnership, LpFund] {
        let c = pipe_of(&EdgeKind::EconomicInterest, Some(t));
        assert_eq!(c.pipe, Some(Pipe::LimitedPartnershipInterest));
        assert!(!c.provisional);
    }
    for t in [OeicIcvc, Sicav, UnitTrust, FortyActFund, UmbrellaWithSubFunds] {
        let c = pipe_of(&EdgeKind::EconomicInterest, Some(t));
        assert_eq!(c.pipe, Some(Pipe::UnitIssuance));
        assert!(!c.provisional);
    }
    // Unknown target type (alleged, not yet proved — CTN-2h weakest-link):
    // provisional AND unresolved — no pipe is invented (D1 corrective
    // tranche Item 4).
    let unknown = pipe_of(&EdgeKind::EconomicInterest, None);
    assert!(unknown.provisional, "no target type ⇒ provisional (CTN-2h)");
    assert_eq!(unknown.pipe, None, "no target type ⇒ no concrete pipe, ever");

    // A known type outside the three ratified buckets — also provisional
    // AND unresolved, an explicitly reported open question, never a
    // silent guess presented as certain.
    let outside_buckets = pipe_of(&EdgeKind::EconomicInterest, Some(GeneralPartnership));
    assert!(
        outside_buckets.provisional,
        "a target type outside TS.2 §3's three ratified buckets must not be presented as certain"
    );
    assert_eq!(
        outside_buckets.pipe, None,
        "a target type outside the three ratified buckets must not invent a concrete pipe"
    );
}

// ── economic_interest_unknown_target_is_provisional (D1 Item 4 gate) ──────

#[test]
fn economic_interest_unknown_target_is_provisional() {
    // Item 4a: no concrete pipe is ever invented for an unresolved
    // EconomicInterest classification — covers both unresolved shapes
    // (no target type at all, and a known target type outside TS.2 §3's
    // three ratified buckets).
    for target in [None, Some(EntityType::GeneralPartnership), Some(EntityType::Foundation)] {
        let c = pipe_of(&EdgeKind::EconomicInterest, target);
        assert!(c.provisional, "target={target:?}: must be provisional");
        assert_eq!(c.pipe, None, "target={target:?}: must carry NO concrete pipe — unresolved, not guessed");
    }

    // Item 4b, the propagation half — reported finding, not silently
    // absorbed: this crate has no "assurance" field (assurance is D2
    // territory per TS.1 §0, out of this tranche's SCOPE FENCE), and the
    // one real D1 consumer of `pipe_of` today —
    // `kyc_ubo.assert.subject.type-correction`'s §4 cascade
    // (`src/domain_ops/kyc_stream_ops.rs::KycSubjectCorrectType`) — turns
    // out to be SAFE regardless of this fix: `NonVotingShares` (the old
    // fabricated default) is geometrically permitted as a pipe-TARGET only
    // into `PrivateLimitedCompany`/`PublicListedCompany`, both already
    // inside TS.2 §3's ratified bucket, and the cascade's SOURCE-correction
    // branch classifies against the (unchanged) far end, not the corrected
    // type — so no live scenario against that one call site can discriminate
    // old from new behavior; an attempted live-DB test proving otherwise
    // (`rust/tests/kyc_ts1_moves_live.rs`) was written, found to assert a
    // false premise under BOTH the old and new code, and deleted rather than
    // left in as a misleading pass. The propagation guarantee that DOES
    // hold, and is checked directly below, is the type-level one: `pipe:
    // Option<Pipe>` makes it a compile error for ANY caller — today's or a
    // future D2/assurance one — to extract a certain `Pipe` from an
    // unresolved classification without an explicit branch.
    let correct_type_src = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../src/domain_ops/kyc_stream_ops.rs"
    ))
    .expect("read kyc_stream_ops.rs");
    let op_start = correct_type_src
        .find("impl SemOsVerbOp for KycSubjectCorrectType {")
        .expect("KycSubjectCorrectType impl must exist");
    let op_end = correct_type_src[op_start..]
        .find("\npub struct KycSubjectWithdrawMember;")
        .map(|i| op_start + i)
        .expect("KycSubjectCorrectType impl must be followed by KycSubjectWithdrawMember");
    let op_body = &correct_type_src[op_start..op_end];
    assert!(
        op_body.contains("match pipe_of(&edge.kind, Some(target_type_for_pipe)).pipe {"),
        "KycSubjectCorrectType must pattern-match pipe_of(..).pipe, not unwrap it — an \
         unresolved (None) classification must be a reachable, handled arm, not a panic \
         or a silent default"
    );
    assert!(
        op_body.contains("Some(pipe) => known_tuples.push"),
        "the Some(pipe) arm must exist — the certain-classification path"
    );
    assert!(
        !op_body.contains(".pipe.unwrap()") && !op_body.contains(".pipe.expect("),
        "KycSubjectCorrectType must never unwrap an unresolved classification"
    );
}

// ── every_geometry_pipe_is_assertable (the tranche's closure tooth) ────────

#[test]
fn every_geometry_pipe_is_assertable() {
    // For every pipe the geometry matrix names, at least one EdgeKind must
    // classify to it (for some target type, or none). Before B1 landed this
    // was RED for six pipes (officer, mandate, membership, statutory,
    // employment, containment) — it is the permanent guard against the
    // split reopening.
    for pipe in ALL_PIPES {
        let assertable = all_edge_kinds().iter().any(|kind| {
            pipe_of(kind, None).pipe == Some(*pipe)
                || ob_poc_kyc_substrate::ALL_ENTITY_TYPES
                    .iter()
                    .any(|t| pipe_of(kind, Some(*t)).pipe == Some(*pipe))
        });
        assert!(assertable, "pipe {pipe:?} has no EdgeKind that classifies to it — unassertable");
    }
}

// ── historical_edges_reclassify_without_mutation ───────────────────────────

#[test]
fn historical_edges_reclassify_without_mutation() {
    // A pre-convergence stream: an EconomicInterest edge asserted before
    // this tranche's growth, into an entity later typed a fund. Folding it
    // (TS.2 §4 read-time classification) must classify correctly with ZERO
    // writes to the stored `EdgeState` — `pipe_of` takes `&EdgeKind` and
    // returns a fresh value; there is no mutation method to call.
    let subject = subject();
    let holder = EntityId(uuid::Uuid::new_v4());
    let fund = EntityId(uuid::Uuid::new_v4());
    let as_of = chrono::DateTime::<chrono::Utc>::from_timestamp(0, 0).unwrap();

    let assert_event = IntentEvent::new(
        subject,
        "kyc_ubo.assert.edge.economic-interest",
        Principal::test_analyst(),
        AuthorityRef("historical".into()),
        TargetBinding::for_subject(subject),
        serde_json::json!({
            "from_entity_id": holder.0, "to_entity_id": fund.0, "percentage": 40.0,
        }),
        as_of,
    );
    let type_event = IntentEvent::new(
        subject,
        "kyc_ubo.assert.subject.type",
        Principal::test_analyst(),
        AuthorityRef("historical".into()),
        TargetBinding { entity_id: Some(fund), ..TargetBinding::for_subject(subject) },
        serde_json::json!({ "entity_id": fund.0, "entity_type": "sicav" }),
        as_of,
    );

    let control = fold_control(&[&assert_event]);
    let type_registry = fold_type_registry(&[&type_event]);

    let edge = control
        .edges
        .values()
        .find(|e| e.from == holder && e.to == fund)
        .expect("historical edge must fold");
    let before: EdgeState = clone_edge(edge);

    let fund_type = type_registry.type_of(fund).expect("type must fold");
    let classification = pipe_of(&edge.kind, Some(fund_type));
    assert_eq!(classification.pipe, Some(Pipe::UnitIssuance), "EconomicInterest into a SICAV is unit issuance");
    assert!(!classification.provisional);

    // No mutation: re-fold the same events, edge is byte-identical.
    let control_again = fold_control(&[&assert_event]);
    let edge_again = control_again
        .edges
        .values()
        .find(|e| e.from == holder && e.to == fund)
        .unwrap();
    assert_eq!(before.id, edge_again.id);
    assert_eq!(before.kind, edge_again.kind);
    assert_eq!(before.status, edge_again.status);
    assert_eq!(before.percentage, edge_again.percentage);
}

fn clone_edge(e: &EdgeState) -> EdgeState {
    EdgeState {
        id: e.id,
        kind: e.kind.clone(),
        from: e.from,
        to: e.to,
        percentage: e.percentage,
        status: e.status,
        evidence_event_id: e.evidence_event_id,
        originating_event_id: e.originating_event_id,
        trust_revocable: e.trust_revocable,
        superseded_by: e.superseded_by,
        pierced_from: e.pierced_from,
    }
}

// ── geometry now reaches the six previously-unassertable pipes ────────────

#[test]
fn geometry_matrix_admits_the_six_newly_assertable_pipes() {
    // Sanity: TS.1's own matrix already declared these pipes permitted —
    // this test confirms the geometry layer (unaffected by B1/B2) still
    // agrees, now that they are assertable.
    assert!(check_type_geometry(
        LinkageSource::Entity(EntityType::NaturalPerson),
        Pipe::OfficerAppointment,
        EntityType::PrivateLimitedCompany,
    )
    .is_ok());
    assert!(check_type_geometry(
        LinkageSource::Entity(EntityType::PrivateLimitedCompany),
        Pipe::ManagementMandate,
        EntityType::LimitedPartnership,
    )
    .is_ok());
    assert!(check_type_geometry(
        LinkageSource::Entity(EntityType::NaturalPerson),
        Pipe::MembershipRights,
        EntityType::CooperativeMutual,
    )
    .is_ok());
    assert!(check_type_geometry(
        LinkageSource::Entity(EntityType::GovernmentDeptStatutoryCorporation),
        Pipe::StatutoryAuthority,
        EntityType::GovernmentDeptStatutoryCorporation,
    )
    .is_ok());
    assert!(check_type_geometry(
        LinkageSource::Entity(EntityType::UmbrellaWithSubFunds),
        Pipe::PooledAssetContainment,
        EntityType::UmbrellaWithSubFunds,
    )
    .is_ok());
    // Employment (14): the TS.1 §2 v0.3 grid gap this test previously
    // pinned as "refused everywhere" was itself a defect, found and fixed
    // in the D1 corrective tranche (2026-08-21) — the document's own
    // "§2 v0.3 corrections" block (dated 2026-08-19, i.e. predates this
    // crate's first geometry.rs transcription) already added pipe 14 to
    // the 8 operating types with employees. Admitted here from a
    // natural-person source into a corporate.
    assert!(check_type_geometry(
        LinkageSource::Entity(EntityType::NaturalPerson),
        Pipe::EmploymentDelegatedAuthority,
        EntityType::PrivateLimitedCompany,
    )
    .is_ok());
    // Deliberately NOT extended to funds (externally managed via pipe 7 —
    // a fund does not employ) — the real, ratified exclusion this test
    // must keep proving now that the false "refused everywhere" pin is
    // gone.
    assert!(check_type_geometry(
        LinkageSource::Entity(EntityType::NaturalPerson),
        Pipe::EmploymentDelegatedAuthority,
        EntityType::Sicav,
    )
    .is_err());
}
