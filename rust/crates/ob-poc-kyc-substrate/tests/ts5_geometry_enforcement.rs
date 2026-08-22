//! EOP-DD-KYCUBO-TS.5 §5 gate suite — pure (no store, no clock). The
//! headline op-level gate and the §4 tooth (`every_geometry_rule_is_
//! reachable_from_the_write_path`) live in `rust/tests/kyc_ts5_geometry_
//! enforcement.rs` (live-DB, drives the real governed op) because TS.5 §5's
//! own text requires them refused "by the governed op, live, not merely by
//! a direct call to the geometry function." Everything here drives
//! `check_preconditions` directly — the identical function the live
//! append path and the placement preview both call (TS.5 §1) — which is
//! the production chokepoint itself, not a bypass of it; it needs no store
//! because `check_preconditions` takes folded state as plain arguments.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use ob_poc_kyc_substrate::{
    check_preconditions, compute_assurance, enumerate_placement_set, fold_control,
    fold_type_registry, assembly_lexicon, AuthorityRef, EdgeId, EntityId, IntentEvent,
    KycError, ObligationState, PersonId, Principal, ProngCandidate, ProvisionalityReason, Prong,
    SubjectId, TargetBinding,
};

fn subject() -> SubjectId {
    SubjectId(Uuid::new_v4())
}

fn as_of() -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(0, 0).unwrap()
}

fn register_event(subject: SubjectId, entity: EntityId) -> IntentEvent {
    IntentEvent::new(
        subject,
        "kyc_ubo.assert.subject.register",
        Principal::test_analyst(),
        AuthorityRef("test".into()),
        TargetBinding::for_subject(subject),
        serde_json::json!({ "entity_id": entity.0.to_string() }),
        as_of(),
    )
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

fn assert_control_event(subject: SubjectId, from: EntityId, to: EntityId, kind: &str) -> IntentEvent {
    IntentEvent::new(
        subject,
        "kyc_ubo.assert.edge.control",
        Principal::test_analyst(),
        AuthorityRef("test".into()),
        TargetBinding::for_edge(subject, EdgeId(Uuid::new_v4())),
        serde_json::json!({
            "from_entity_id": from.0.to_string(),
            "to_entity_id": to.0.to_string(),
            "kind": kind,
        }),
        as_of(),
    )
}

fn correct_type_event(subject: SubjectId, entity: EntityId, wire: &str, invalidated: &[EdgeId]) -> IntentEvent {
    IntentEvent::new(
        subject,
        "kyc_ubo.assert.subject.type-correction",
        Principal::test_analyst(),
        AuthorityRef("test".into()),
        TargetBinding { entity_id: Some(entity), ..TargetBinding::for_subject(subject) },
        serde_json::json!({
            "entity_id": entity.0.to_string(),
            "entity_type": wire,
            "invalidated_edge_ids": invalidated.iter().map(|e| e.0.to_string()).collect::<Vec<_>>(),
        }),
        as_of(),
    )
}

/// TS.1 §2, the target side: a voting-share linkage into a discretionary
/// trust is not a move — the trust's target row (§2) admits only 8/9/10
/// (trustee/reserved/beneficiary), never 1 (voting shares). Source is a
/// NATURAL PERSON, legal for pipe 1 — isolating the TARGET restriction.
#[test]
fn illegal_target_type_is_refused() {
    let subject = subject();
    let person = EntityId(Uuid::new_v4());
    let trust = EntityId(Uuid::new_v4());
    let reg1 = register_event(subject, person);
    let reg2 = register_event(subject, trust);
    let t1 = assert_type_event(subject, person, "natural_person");
    let t2 = assert_type_event(subject, trust, "discretionary_trust");
    let control = fold_control(&[&reg1, &reg2, &t1, &t2]);
    let type_registry = fold_type_registry(&[&reg1, &reg2, &t1, &t2]);
    let obligation = ObligationState::default();

    let lexicon = assembly_lexicon();
    let entry = lexicon.get("kyc_ubo.assert.edge.control").unwrap();
    let event = assert_control_event(subject, person, trust, "voting_rights");
    let verdict = check_preconditions(entry, &control, &obligation, &type_registry, &event);
    assert!(matches!(verdict, Err(KycError::GeometryRefused { .. })), "got {verdict:?}");
}

/// TS.5 R2 — property: the board offers `kyc_ubo.assert.edge.control` iff at
/// least one real (from, kind, to) triple among registered/typed members
/// would be admitted by `check_preconditions` (the identical function). Two
/// halves: a board with NO legal pair anywhere hides the verb AND refuses a
/// direct real submission the same way; a board WITH a legal pair offers
/// the verb AND admits that specific real submission.
#[test]
fn preview_and_append_agree() {
    let lexicon = assembly_lexicon();
    let entry = lexicon.get("kyc_ubo.assert.edge.control").unwrap();

    // Half 1 — a board on which the GEOMETRY-REFUSED kinds must be refused by
    // both sides. Two NaturalPerson-typed members: a person is never a target
    // (TS.1 §2), so `voting_rights` cannot land on either as `to`.
    //
    // TS.5 R2 (2026-08-22): this half previously asserted the board offers
    // `assert-control` NOWHERE here, on the premise "no legal pair exists".
    // That premise was FALSE and the assertion was itself an instance of the
    // divergence R2 exists to prevent — the append ADMITS
    // `person --economic_interest--> person2` on this very board (Unevaluable,
    // R6/CTN-2e; see `illegal_triple_is_not_offered`). The existence scan said
    // "no move" while the append said "admit", and the gate recorded that as
    // correct. Reported before fixed, per R4. It now asserts the AGREEMENT
    // property per triple, which is what R2 actually rules.
    {
        let subject = subject();
        let a = EntityId(Uuid::new_v4());
        let b = EntityId(Uuid::new_v4());
        let reg1 = register_event(subject, a);
        let reg2 = register_event(subject, b);
        let t1 = assert_type_event(subject, a, "natural_person");
        let t2 = assert_type_event(subject, b, "natural_person");
        let control = fold_control(&[&reg1, &reg2, &t1, &t2]);
        let type_registry = fold_type_registry(&[&reg1, &reg2, &t1, &t2]);
        let obligation = ObligationState::default();

        let board = enumerate_placement_set(subject, &control, &obligation, &type_registry, &lexicon);
        let offered_voting = board
            .moves
            .iter()
            .filter(|m| m.verb_fqn.0 == "kyc_ubo.assert.edge.control")
            .filter_map(|m| m.proposed_edge.as_ref())
            .any(|t| t.kind_wire == "voting_rights");
        assert!(
            !offered_voting,
            "board must not offer a voting_rights triple — a person is never a target"
        );

        let event = assert_control_event(subject, a, b, "voting_rights");
        let verdict = check_preconditions(entry, &control, &obligation, &type_registry, &event);
        assert!(verdict.is_err(), "append should ALSO refuse — preview and append agree (refuse case)");
    }

    // Half 2 — a legal pair exists: NaturalPerson --VotingShares--> PrivateLimitedCompany.
    {
        let subject = subject();
        let person = EntityId(Uuid::new_v4());
        let corp = EntityId(Uuid::new_v4());
        let reg1 = register_event(subject, person);
        let reg2 = register_event(subject, corp);
        let t1 = assert_type_event(subject, person, "natural_person");
        let t2 = assert_type_event(subject, corp, "private_limited_company");
        let control = fold_control(&[&reg1, &reg2, &t1, &t2]);
        let type_registry = fold_type_registry(&[&reg1, &reg2, &t1, &t2]);
        let obligation = ObligationState::default();

        let board = enumerate_placement_set(subject, &control, &obligation, &type_registry, &lexicon);
        // The SPECIFIC triple, not merely the verb — the whole point of R2.
        let offered = board
            .moves
            .iter()
            .filter(|m| m.verb_fqn.0 == "kyc_ubo.assert.edge.control")
            .filter_map(|m| m.proposed_edge.as_ref())
            .any(|t| t.from == person && t.to == corp && t.kind_wire == "voting_rights");
        assert!(offered, "board SHOULD offer person --voting_rights--> corp");

        let event = assert_control_event(subject, person, corp, "voting_rights");
        let verdict = check_preconditions(entry, &control, &obligation, &type_registry, &event);
        assert!(verdict.is_ok(), "append should ALSO admit this specific legal triple: {verdict:?}");
    }
}

/// TS.5 R6 — an endpoint with NO type asserted at all admits, and the
/// determination that traverses it is provisional
/// (`ProvisionalityReason::GeometryUnevaluable`), never a fabricated pipe.
#[test]
fn untyped_endpoint_admits_provisionally() {
    let subject = subject();
    let a = EntityId(Uuid::new_v4());
    let b = EntityId(Uuid::new_v4());
    let reg1 = register_event(subject, a);
    let reg2 = register_event(subject, b);
    // Deliberately NO assert-type for either entity.
    let control = fold_control(&[&reg1, &reg2]);
    let type_registry = fold_type_registry(&[&reg1, &reg2]);
    let obligation = ObligationState::default();

    let lexicon = assembly_lexicon();
    let entry = lexicon.get("kyc_ubo.assert.edge.control").unwrap();
    let assert_event = assert_control_event(subject, a, b, "voting_rights");
    let verdict = check_preconditions(entry, &control, &obligation, &type_registry, &assert_event);
    assert!(verdict.is_ok(), "untyped endpoint must ADMIT, not refuse: {verdict:?}");

    // Fold the real event so the edge id is deterministic-per-payload, then
    // build a determination touching `b` and confirm assurance records it.
    let control2 = fold_control(&[&reg1, &reg2, &assert_event]);
    let edge_id = *control2.edges.keys().next().expect("edge inserted");
    let candidate = ProngCandidate {
        person_id: PersonId(a.0),
        prong: Prong::ControlByOtherMeans,
        effective_ownership_pct: None,
        ownership_chain: vec![b],
        originating_event_id: assert_event.id,
        pivot: None,
        pierces: Vec::new(),
    };
    let _ = edge_id; // sanity: edge really landed
    let assurance = compute_assurance(&[candidate], &[], &control2, &type_registry);
    assert!(
        assurance.reasons.iter().any(|r| matches!(r, ProvisionalityReason::GeometryUnevaluable { .. })),
        "expected GeometryUnevaluable in {:?}",
        assurance.reasons
    );
}

/// TS.5 R5 — an endpoint typed but only ALLEGED (not yet evidenced) admits,
/// and the pre-existing `ProvisionalityReason::AllegedType` mechanism
/// (TS.3) already propagates it — R1 needed no new plumbing for this half.
#[test]
fn alleged_type_admits_provisionally() {
    let subject = subject();
    let person = EntityId(Uuid::new_v4());
    let corp = EntityId(Uuid::new_v4());
    let reg1 = register_event(subject, person);
    let reg2 = register_event(subject, corp);
    let t1 = assert_type_event(subject, person, "natural_person");
    let t2 = assert_type_event(subject, corp, "private_limited_company");
    // No attach-evidence — both types stay Alleged, never Proved.
    let control = fold_control(&[&reg1, &reg2, &t1, &t2]);
    let type_registry = fold_type_registry(&[&reg1, &reg2, &t1, &t2]);
    let obligation = ObligationState::default();

    let lexicon = assembly_lexicon();
    let entry = lexicon.get("kyc_ubo.assert.edge.control").unwrap();
    let assert_event = assert_control_event(subject, person, corp, "voting_rights");
    let verdict = check_preconditions(entry, &control, &obligation, &type_registry, &assert_event);
    assert!(verdict.is_ok(), "alleged-typed endpoint must ADMIT: {verdict:?}");

    let candidate = ProngCandidate {
        person_id: PersonId(person.0),
        prong: Prong::ControlByOtherMeans,
        effective_ownership_pct: None,
        ownership_chain: vec![corp],
        originating_event_id: assert_event.id,
        pivot: None,
        pierces: Vec::new(),
    };
    let assurance = compute_assurance(&[candidate], &[], &control, &type_registry);
    assert!(
        assurance.reasons.iter().any(|r| matches!(r, ProvisionalityReason::AllegedType { .. })),
        "expected AllegedType in {:?}",
        assurance.reasons
    );
}

/// TS.5 §5 — the two constraint layers (TS.1 §1) must be distinguishable
/// errors: "not a move" (geometry) versus "not legal here" (a stud).
#[test]
fn geometry_refusal_is_distinguishable_from_stud_refusal() {
    let lexicon = assembly_lexicon();
    let entry = lexicon.get("kyc_ubo.assert.edge.control").unwrap();

    // Geometry violation: ManagementMandate sourced from a natural person.
    let subject = subject();
    let person = EntityId(Uuid::new_v4());
    let fund = EntityId(Uuid::new_v4());
    let reg1 = register_event(subject, person);
    let reg2 = register_event(subject, fund);
    let t1 = assert_type_event(subject, person, "natural_person");
    let t2 = assert_type_event(subject, fund, "oeic_icvc");
    let control = fold_control(&[&reg1, &reg2, &t1, &t2]);
    let type_registry = fold_type_registry(&[&reg1, &reg2, &t1, &t2]);
    let obligation = ObligationState::default();
    let geo_event = assert_control_event(subject, person, fund, "management_mandate");
    let geo_verdict = check_preconditions(entry, &control, &obligation, &type_registry, &geo_event);
    assert!(matches!(geo_verdict, Err(KycError::GeometryRefused { .. })), "got {geo_verdict:?}");

    // Stud violation: a duplicate active edge of the same (from, to, kind) —
    // NoDuplicateActiveEdge (K-13), nothing to do with geometry.
    let subject_b = SubjectId(Uuid::new_v4());
    let corp_a = EntityId(Uuid::new_v4());
    let corp_b = EntityId(Uuid::new_v4());
    let reg3 = register_event(subject_b, corp_a);
    let reg4 = register_event(subject_b, corp_b);
    let t3 = assert_type_event(subject_b, corp_a, "natural_person");
    let t4 = assert_type_event(subject_b, corp_b, "private_limited_company");
    let first_assert = assert_control_event(subject_b, corp_a, corp_b, "voting_rights");
    let control2 = fold_control(&[&reg3, &reg4, &t3, &t4, &first_assert]);
    let type_registry2 = fold_type_registry(&[&reg3, &reg4, &t3, &t4]);
    let second_assert = assert_control_event(subject_b, corp_a, corp_b, "voting_rights");
    let stud_verdict = check_preconditions(entry, &control2, &obligation, &type_registry2, &second_assert);
    assert!(matches!(stud_verdict, Err(KycError::PreconditionFailed { .. })), "got {stud_verdict:?}");

    // Distinguishable in both directions.
    assert!(!matches!(geo_verdict, Err(KycError::PreconditionFailed { .. })));
    assert!(!matches!(stud_verdict, Err(KycError::GeometryRefused { .. })));
}

/// TS.5 regression guard: R1/R2 add a NEW `TypeGeometryPermits` call site;
/// they must not disturb the EXISTING `correct-type` cascade
/// (`edges_invalidated_by_correction`), which stays the sole place
/// `check_type_geometry` runs post-hoc rather than at assertion time.
#[test]
fn type_correction_still_cascades() {
    let subject = subject();
    let corp = EntityId(Uuid::new_v4());
    let person = EntityId(Uuid::new_v4());
    let reg1 = register_event(subject, corp);
    let reg2 = register_event(subject, person);
    let t1 = assert_type_event(subject, corp, "private_limited_company");
    let t2 = assert_type_event(subject, person, "natural_person");
    // BoardAppointment is legal into a PrivateLimitedCompany from a person.
    let board_edge = assert_control_event(subject, person, corp, "board_appointment");
    let control = fold_control(&[&reg1, &reg2, &t1, &t2, &board_edge]);
    let edge_id = *control.edges.keys().next().expect("edge landed");

    let touching = [(edge_id, ob_poc_kyc_substrate::pipe_of(&control.edges[&edge_id].kind, Some(ob_poc_kyc_substrate::EntityType::PrivateLimitedCompany)).pipe.unwrap(), ob_poc_kyc_substrate::EntityType::PrivateLimitedCompany, true)];
    // Correcting `corp` to a Sicav: BoardAppointment is not in a Sicav's
    // target row (§2) — the edge must be invalidated.
    let invalidated = ob_poc_kyc_substrate::edges_invalidated_by_correction(
        ob_poc_kyc_substrate::EntityType::Sicav,
        &touching,
    );
    assert_eq!(invalidated, vec![edge_id], "correct-type cascade regressed");

    let correction = correct_type_event(subject, corp, "sicav", &invalidated);
    let type_registry2 = fold_type_registry(&[&reg1, &reg2, &t1, &t2, &correction]);
    assert!(type_registry2.determination_stale, "correction must mark the determination stale");
}

// ── TS.5 R2, closed 2026-08-22 ──────────────────────────────────────────────
//
// The 2026-08-22 reconciliation proved R2 was NOT closed: `geometrically_
// possible` took no (from, kind, to) and asked only "does ANY legal triple
// exist among typed members anywhere". `preview_and_append_agree` above passed
// because it tests only the two EXTREMES — a board with no legal pair, and a
// board with a legal pair which it then submits. It never submits a DIFFERENT
// triple on a board that passed the existence gate, which is the only shape
// that can catch the defect.

/// Build a MIXED board: person, person2 (both natural persons) and corp.
/// `person --VotingShares--> corp` is legal; `person --VotingRights--> person2`
/// is not (a person is never a target, TS.1 §2).
fn mixed_board() -> (SubjectId, EntityId, EntityId, EntityId, Vec<IntentEvent>) {
    let subject = subject();
    let person = EntityId(Uuid::new_v4());
    let person2 = EntityId(Uuid::new_v4());
    let corp = EntityId(Uuid::new_v4());
    let evs = vec![
        register_event(subject, person),
        register_event(subject, person2),
        register_event(subject, corp),
        assert_type_event(subject, person, "natural_person"),
        assert_type_event(subject, person2, "natural_person"),
        assert_type_event(subject, corp, "private_limited_company"),
    ];
    (subject, person, person2, corp, evs)
}

/// The specific case the reconciliation reproduced: on a board where a legal
/// pair exists, the illegal triple must NOT be offered.
#[test]
fn illegal_triple_is_not_offered() {
    let lexicon = assembly_lexicon();
    let (subject, person, person2, corp, evs) = mixed_board();
    let refs: Vec<&IntentEvent> = evs.iter().collect();
    let control = fold_control(&refs);
    let type_registry = fold_type_registry(&refs);
    let obligation = ObligationState::default();

    let board = enumerate_placement_set(subject, &control, &obligation, &type_registry, &lexicon);

    let offered_triples: Vec<(EntityId, EntityId, String)> = board
        .moves
        .iter()
        .filter(|m| m.verb_fqn.0 == "kyc_ubo.assert.edge.control")
        .filter_map(|m| m.proposed_edge.as_ref())
        .map(|t| (t.from, t.to, t.kind_wire.clone()))
        .collect();

    assert!(
        !offered_triples.is_empty(),
        "the board must offer the legal triple(s) — a gate that offers nothing \
         is not a fix"
    );
    assert!(
        offered_triples
            .iter()
            .any(|(f, t, k)| *f == person && *t == corp && k == "voting_rights"),
        "person --voting_rights--> corp is legal and must be offered; got {offered_triples:?}"
    );
    // Scoped to GEOMETRY-REFUSED kinds, not "any kind". `economic_interest`
    // into a natural person is deliberately NOT refused: TS.2 §3 names no pipe
    // for it, so `classify_economic_interest` returns no pipe at all and
    // `evaluate_type_geometry` reports `Unevaluable`, which ADMITS under
    // R6/CTN-2e. D1 corrective-tranche Item 4 (2026-08-21) made that explicit —
    // "an open question, not silently resolved" — after it had previously
    // defaulted to `NonVotingShares`, a guess dressed up as a flagged value.
    // Asserting the board must offer NOTHING person→person would therefore be
    // asserting against a ratified ruling, not for R2.
    assert!(
        !offered_triples
            .iter()
            .any(|(f, t, k)| *f == person && *t == person2 && k == "voting_rights"),
        "person --voting_rights--> person2 is NOT a move (person is never a \
         target, TS.1 §2) and must not be offered; got {offered_triples:?}"
    );
}

/// The gate `preview_and_append_agree` should have been: over a board holding
/// BOTH legal and illegal pairs, for EVERY candidate triple the board offers a
/// move IFF the append admits it. Both directions — an offered-but-refused
/// triple is the preview-vs-reality divergence R2 exists to prevent, and an
/// admitted-but-unoffered triple is a board hiding a legal move.
#[test]
fn preview_and_append_agree_for_every_triple() {
    use ob_poc_kyc_substrate::EDGE_KIND_WIRE_VALUES;

    let lexicon = assembly_lexicon();
    let entry = lexicon.get("kyc_ubo.assert.edge.control").unwrap();
    let (subject, person, person2, corp, evs) = mixed_board();
    let refs: Vec<&IntentEvent> = evs.iter().collect();
    let control = fold_control(&refs);
    let type_registry = fold_type_registry(&refs);
    let obligation = ObligationState::default();

    let board = enumerate_placement_set(subject, &control, &obligation, &type_registry, &lexicon);
    let offered: std::collections::BTreeSet<(EntityId, EntityId, String)> = board
        .moves
        .iter()
        .filter(|m| m.verb_fqn.0 == "kyc_ubo.assert.edge.control")
        .filter_map(|m| m.proposed_edge.as_ref())
        .map(|t| (t.from, t.to, t.kind_wire.clone()))
        .collect();

    let members = [person, person2, corp];
    let mut disagreements: Vec<String> = Vec::new();
    let mut checked = 0usize;
    for &from in &members {
        for &to in &members {
            if from == to {
                continue;
            }
            for wire in EDGE_KIND_WIRE_VALUES {
                checked += 1;
                let is_offered = offered.contains(&(from, to, (*wire).to_string()));
                let event = assert_control_event(subject, from, to, wire);
                let admits =
                    check_preconditions(entry, &control, &obligation, &type_registry, &event)
                        .is_ok();
                if is_offered != admits {
                    disagreements.push(format!(
                        "{from:?} --{wire}--> {to:?}: offered={is_offered} admitted={admits}"
                    ));
                }
            }
        }
    }
    assert!(checked >= 6 * 17, "the sample must be the full triple space; checked {checked}");
    assert!(
        disagreements.is_empty(),
        "preview and append must agree for EVERY triple (TS.5 R2), not just the \
         two extremes; {} of {checked} disagreed:\n{}",
        disagreements.len(),
        disagreements.join("\n")
    );
}
