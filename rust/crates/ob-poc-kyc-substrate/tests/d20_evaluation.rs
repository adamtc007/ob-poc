//! EOP-DD-KYCUBO-D2.0 gate suite — the evaluation pack's foundation. Pure
//! (no store, no clock), same discipline as `ts5_geometry_enforcement.rs`:
//! folded state is plain data, so these gates build it directly rather than
//! replaying events through the fold.
//!
//! **EOP-DD-KYCUBO-D2.1 Tranche B (2026-08-23) rehomed five of the original
//! nine D2.0 §6 gates** onto the production `SemOsVerbOp::execute` path —
//! `checks_run_at_any_board_state`, `in_scope_set_is_computed_not_stored`,
//! `staleness_is_hash_comparison`, `runs_are_append_only`,
//! `run_pins_are_complete` — see
//! `rust/tests/kyc_d21_gate_rehoming.rs::*_through_production`. The
//! reconciliation that triggered the rehoming proved these pure-fixture
//! versions could not detect production writing a FABRICATED in-scope set
//! (they build their own `TestCheck` fixtures and never read anything
//! production persisted), so they no longer serve as this programme's proof
//! that the property holds in the running system.
//!
//! Four are removed outright, not duplicated, to avoid two suites quietly
//! drifting apart. `run_pins_are_complete` is the one exception: it names
//! two genuinely distinct properties that were bundled under one test —
//! "does the pure constructor (`EvaluationRun::new`) refuse an incomplete
//! pin set" (constructor-level, retained here as
//! `evaluation_run_new_refuses_incomplete_pins`, and does not overlap with
//! anything the production gate checks) and "does production actually call
//! that constructor with complete, real pins" (rehomed as
//! `run_pins_are_complete_through_production`). Splitting rather than
//! moving is the correct call only because the two halves test different
//! things; it is not the default and is called out explicitly so it is not
//! read as quiet drift.
//!
//! The remaining two — `unevaluable_is_not_fail` and
//! `work_list_is_derived_from_latest_run` — carry a D2.1 §3 four-field
//! deferral notice each: production cannot produce a real verdict at all
//! today (`Check` has no `evaluate()`, the catalogue is a literal `&[]`),
//! so there is nothing for a production-driven version of either gate to
//! observe. They stay here as pure-function coverage until Tranche C.

use uuid::Uuid;

use ob_poc_kyc_substrate::{
    applicability_holds, work_list_from_history, ApplicabilityCondition, BoardSnapshot,
    ControlState, EntityId, EntityType, EvaluationRun, EventId, Finding, KycError, RunPins,
    RunTrigger, SubjectId, TypeRegistryState, Verdict,
};

fn empty_board<'a>(control: &'a ControlState, types: &'a TypeRegistryState) -> BoardSnapshot<'a> {
    BoardSnapshot { control, type_registry: types, determination: None }
}

/// The two board-gap condition kinds (P0 0c): typed and total, but always
/// absent against today's board since no jurisdiction or risk-rating source
/// exists yet. This is the board's gap, not a bug in the condition kind.
#[test]
fn jurisdiction_and_risk_conditions_are_typed_but_currently_unsatisfiable() {
    let control = ControlState::default();
    let types = TypeRegistryState::default();
    let board = empty_board(&control, &types);

    assert!(!ApplicabilityCondition::JurisdictionPresent.holds(&board));
    assert!(!ApplicabilityCondition::RiskAtOrAbove(ob_poc_kyc_substrate::RiskLevel::Low).holds(&board));
}

#[test]
fn entity_type_present_condition_reads_the_type_registry() {
    use ob_poc_kyc_substrate::{fold_type_registry, AuthorityRef, IntentEvent, Principal, SubjectId, TargetBinding};
    use chrono::{DateTime, Utc};

    fn as_of() -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(0, 0).unwrap()
    }

    let subject = SubjectId(Uuid::new_v4());
    let entity = EntityId(Uuid::new_v4());
    let reg = IntentEvent::new(
        subject,
        "kyc_ubo.assert.subject.register",
        Principal::test_analyst(),
        AuthorityRef("test".into()),
        TargetBinding::for_subject(subject),
        serde_json::json!({ "entity_id": entity.0.to_string() }),
        as_of(),
    );
    let assert_type = IntentEvent::new(
        subject,
        "kyc_ubo.assert.subject.type",
        Principal::test_analyst(),
        AuthorityRef("test".into()),
        TargetBinding { entity_id: Some(entity), ..TargetBinding::for_subject(subject) },
        serde_json::json!({ "entity_id": entity.0.to_string(), "entity_type": "discretionary_trust" }),
        as_of(),
    );
    let control = ControlState::default();
    let types = fold_type_registry(&[&reg, &assert_type]);
    let board = empty_board(&control, &types);

    assert!(
        ApplicabilityCondition::EntityTypePresent(EntityType::DiscretionaryTrust).holds(&board)
    );
    assert!(!ApplicabilityCondition::EntityTypePresent(EntityType::Foundation).holds(&board));
    assert!(applicability_holds(
        &[ApplicabilityCondition::EntityTypePresent(EntityType::DiscretionaryTrust)],
        &board
    ));
}

// ── D2.1 §2/§7 Q2 — `ProvenTypeCheck`, the one real proof check ────────────

use ob_poc_kyc_substrate::{Check, ProvenTypeCheck};

#[test]
fn proven_type_check_passes_on_empty_board() {
    let control = ControlState::default();
    let types = TypeRegistryState::default();
    let board = empty_board(&control, &types);
    let outcome = ProvenTypeCheck.evaluate(&board);
    assert!(matches!(outcome.verdict, Verdict::Pass), "vacuously true — nothing to fail on");
    assert!(
        outcome.cites.is_empty(),
        "a vacuous Pass (zero registered entities) must cite nothing — the mechanism-level half of \
         `a_vacuous_pass_is_distinguishable`, at the pure `evaluate()` layer"
    );
}

#[test]
fn proven_type_check_passes_when_every_entity_is_proved() {
    use ob_poc_kyc_substrate::{fold_type_registry, AuthorityRef, IntentEvent, Principal, SubjectId, TargetBinding};
    use chrono::{DateTime, Utc};
    fn as_of() -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(0, 0).unwrap()
    }

    let subject = SubjectId(Uuid::new_v4());
    let entity = EntityId(Uuid::new_v4());
    let assert_type = IntentEvent::new(
        subject,
        "kyc_ubo.assert.subject.type",
        Principal::test_analyst(),
        AuthorityRef("test".into()),
        TargetBinding { entity_id: Some(entity), ..TargetBinding::for_subject(subject) },
        serde_json::json!({ "entity_id": entity.0.to_string(), "entity_type": "natural_person" }),
        as_of(),
    );
    let evidence = IntentEvent::new(
        subject,
        "kyc_ubo.assert.edge.evidence",
        Principal::test_analyst(),
        AuthorityRef("test".into()),
        TargetBinding { entity_id: Some(entity), ..TargetBinding::for_subject(subject) },
        serde_json::json!({
            "entity_id": entity.0.to_string(),
            "kind": "identity-document",
            "source": "test fixture",
            "date": "2026-08-28",
        }),
        as_of(),
    );
    let types = fold_type_registry(&[&assert_type, &evidence]);
    let mut control = ControlState::default();
    control.registered_entity_ids.insert(entity);
    let board = empty_board(&control, &types);
    let outcome = ProvenTypeCheck.evaluate(&board);
    let evidence_event_ids = types.proof_event_ids_of(entity);
    assert_eq!(
        outcome.cites,
        evidence_event_ids,
        "a real Pass must cite the exact events that proved the entity's type — the mechanism-level \
         half of `a_real_pass_cites_its_evidence`, at the pure `evaluate()` layer"
    );
    assert!(
        matches!(outcome.verdict, Verdict::Pass),
        "a type-scoped evidence event must fold a proof against the entity's type"
    );
}

#[test]
fn proven_type_check_is_unevaluable_when_a_type_is_alleged() {
    use ob_poc_kyc_substrate::{fold_type_registry, AuthorityRef, IntentEvent, Principal, SubjectId, TargetBinding};
    use chrono::{DateTime, Utc};
    fn as_of() -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(0, 0).unwrap()
    }

    let subject = SubjectId(Uuid::new_v4());
    let entity = EntityId(Uuid::new_v4());
    let assert_type = IntentEvent::new(
        subject,
        "kyc_ubo.assert.subject.type",
        Principal::test_analyst(),
        AuthorityRef("test".into()),
        TargetBinding { entity_id: Some(entity), ..TargetBinding::for_subject(subject) },
        serde_json::json!({ "entity_id": entity.0.to_string(), "entity_type": "natural_person" }),
        as_of(),
    );
    let types = fold_type_registry(&[&assert_type]);
    let mut control = ControlState::default();
    control.registered_entity_ids.insert(entity);
    let board = empty_board(&control, &types);
    let outcome = ProvenTypeCheck.evaluate(&board);
    assert_eq!(
        outcome.cites,
        vec![assert_type.id],
        "Unevaluable(AllegedType) must cite the assertion event that made the type Alleged — \
         the mechanism-level half of `alleged_finding_cites_its_assertion`"
    );
    match outcome.verdict {
        Verdict::Unevaluable { reason: ob_poc_kyc_substrate::UnevaluableReason::Provisional(_) } => {}
        other => panic!("expected Unevaluable(Provisional(AllegedType)), got {other:?}"),
    }
}

#[test]
fn proven_type_check_is_unevaluable_when_no_type_is_asserted_at_all() {
    let mut control = ControlState::default();
    let entity = EntityId(Uuid::new_v4());
    control.registered_entity_ids.insert(entity);
    let types = TypeRegistryState::default();
    let board = empty_board(&control, &types);
    match ProvenTypeCheck.evaluate(&board).verdict {
        Verdict::Unevaluable { reason: ob_poc_kyc_substrate::UnevaluableReason::FactAbsent { .. } } => {}
        other => panic!("expected Unevaluable(FactAbsent), got {other:?}"),
    }
}

#[test]
fn proven_type_check_fails_on_a_withdrawn_unproven_entity() {
    let mut control = ControlState::default();
    let entity = EntityId(Uuid::new_v4());
    control.registered_entity_ids.insert(entity);
    let mut types = TypeRegistryState::default();
    let withdrawal_event = EventId(Uuid::new_v4());
    types.withdrawn_members.insert(entity, withdrawal_event);
    let board = empty_board(&control, &types);
    let outcome = ProvenTypeCheck.evaluate(&board);
    match outcome.verdict {
        Verdict::Fail { detail } => assert!(detail.contains(&entity.0.to_string()), "detail must name the offending entity"),
        other => panic!("expected Fail, got {other:?}"),
    }
    assert!(
        outcome.cites.contains(&withdrawal_event),
        "Fail must cite the withdrawal event that caused it — the mechanism-level half \
         of `a_fail_cites_its_withdrawal`: {:?}",
        outcome.cites
    );
}

// ── §4/§6 run book gates — pure-function coverage retained under a D2.1 §3
// deferral (see the module doc comment) ─────────────────────────────────────

fn pins(check_ids: Vec<String>) -> RunPins {
    use chrono::{DateTime, Utc};
    use ob_poc_kyc_substrate::Hash;
    fn as_of() -> DateTime<Utc> {
        DateTime::<Utc>::from_timestamp(0, 0).unwrap()
    }
    RunPins {
        subject_root: ob_poc_kyc_substrate::SubjectId(Uuid::new_v4()),
        board_state_hash: Hash::of_json(&serde_json::json!({"n": 1})),
        evaluation_pack_version_hash: Hash::of_json(&serde_json::json!({"v": 1})),
        valid_time: as_of(),
        knowledge_time: as_of(),
        trigger: RunTrigger { verb_fqn: "test".into(), session_id: Uuid::new_v4() },
        in_scope_check_ids: check_ids,
    }
}

/// D2.0 §6 `unevaluable_is_not_fail` — property: adding the missing fact and
/// re-running flips it to pass or fail, never the reverse.
///
/// D2.1 §3 DEFERRAL — not rehomed onto production (Tranche B):
///   WHAT: proving through a real op call that a check whose fact is absent
///         persists `Verdict::Unevaluable` (never `Fail`), and that
///         supplying the fact and re-running flips it — never the reverse.
///   WHY:  no production path can produce ANY verdict today — `Check` has
///         no `evaluate()`, the catalogue passed to every decide op is a
///         literal `&[]`, and `findings` is hardcoded `json!([])` at the
///         persist site. There is nothing for a production-driven version
///         of this gate to observe.
///   WHO:  D2.1 Tranche C (`Check::evaluate`, the real catalogue, and real
///         findings persistence) — same task this deferral was recorded in.
///   WHEN: D2.1 Tranche C.
#[test]
fn unevaluable_is_not_fail() {
    let subject = SubjectId(Uuid::new_v4());
    let unevaluable = Finding::unevaluable(
        "sanctions.screen",
        subject,
        ob_poc_kyc_substrate::UnevaluableReason::FactAbsent { what: "no screening fact".into() },
    );
    assert!(matches!(unevaluable.verdict, Verdict::Unevaluable { .. }));
    assert!(!matches!(unevaluable.verdict, Verdict::Fail { .. }));

    // Re-running with the fact now present flips it — never the reverse.
    let now_pass = Finding::pass("sanctions.screen", subject, vec![]);
    assert!(matches!(now_pass.verdict, Verdict::Pass));
}

/// D2.0 §6 `work_list_is_derived_from_latest_run` — structural: no stored
/// work-list state exists; the list is a query over the newest run.
///
/// D2.1 §3 DEFERRAL — not rehomed onto production (Tranche B):
///   WHAT: proving through real persisted run history that the current
///         work list is the LATEST run's failing/unevaluable findings,
///         ignoring a stale prior run's findings.
///   WHY:  same root cause as `unevaluable_is_not_fail` above — findings
///         are always `[]` in every production run today, so two real runs
///         are indistinguishable by work list regardless of ordering; the
///         property has nothing to exercise through production yet.
///   WHO:  D2.1 Tranche C — same task this deferral was recorded in.
///   WHEN: D2.1 Tranche C.
#[test]
fn work_list_is_derived_from_latest_run() {
    let subject = SubjectId(Uuid::new_v4());
    let old_run = EvaluationRun::new(
        Uuid::new_v4(),
        pins(vec!["c1".into()]),
        vec![Finding::fail("c1", subject, "was failing", vec![])],
    )
    .unwrap();
    let new_run = EvaluationRun::new(
        Uuid::new_v4(),
        pins(vec!["c1".into()]),
        vec![Finding::pass("c1", subject, vec![])],
    )
    .unwrap();

    let history = vec![old_run, new_run];
    let work_list = work_list_from_history(&history);
    assert!(work_list.is_empty(), "the fix in the newest run must clear the work list, ignoring the stale old run");
}

/// Retained as a pure-function sanity check on `EvaluationRun::new`'s
/// refusal path — `run_pins_are_complete` itself was rehomed onto
/// production (`rust/tests/kyc_d21_gate_rehoming.rs::run_pins_are_complete_through_production`),
/// but this constructor-level check is cheap, fast, and does not overlap
/// with what that gate proves (the production gate checks what actually
/// got persisted; this one checks the constructor refuses before anything
/// would be persisted).
#[test]
fn evaluation_run_new_refuses_incomplete_pins() {
    let missing_trigger = RunPins {
        trigger: RunTrigger { verb_fqn: "".into(), session_id: Uuid::new_v4() },
        ..pins(vec!["c1".into()])
    };
    let err = EvaluationRun::new(Uuid::new_v4(), missing_trigger, vec![]).unwrap_err();
    assert!(matches!(err, KycError::IncompleteRun { .. }));

    let missing_session = RunPins {
        trigger: RunTrigger { verb_fqn: "test".into(), session_id: Uuid::nil() },
        ..pins(vec!["c1".into()])
    };
    let err_session = EvaluationRun::new(Uuid::new_v4(), missing_session, vec![]).unwrap_err();
    assert!(matches!(err_session, KycError::IncompleteRun { .. }));

    let missing_subject =
        RunPins { subject_root: ob_poc_kyc_substrate::SubjectId(Uuid::nil()), ..pins(vec!["c1".into()]) };
    let err2 = EvaluationRun::new(Uuid::new_v4(), missing_subject, vec![]).unwrap_err();
    assert!(matches!(err2, KycError::IncompleteRun { .. }));

    // Complete pins: construction succeeds, including a run with an EMPTY
    // in-scope set (a legitimate computed value, not a missing pin).
    assert!(EvaluationRun::new(Uuid::new_v4(), pins(vec!["c1".into()]), vec![]).is_ok());
    assert!(EvaluationRun::new(Uuid::new_v4(), pins(vec![]), vec![]).is_ok());
}
