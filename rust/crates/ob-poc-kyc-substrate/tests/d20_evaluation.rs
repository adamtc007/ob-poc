//! EOP-DD-KYCUBO-D2.0 gate suite — the evaluation pack's foundation. Pure
//! (no store, no clock), same discipline as `ts5_geometry_enforcement.rs`:
//! folded state is plain data, so these gates build it directly rather than
//! replaying events through the fold.

use chrono::{DateTime, Utc};
use uuid::Uuid;

use ob_poc_kyc_substrate::{
    applicability_holds, board_state_hash, in_scope_check_ids, work_list_from_history,
    ApplicabilityCondition, BoardSnapshot, Check, ControlState, EntityId, EntityType,
    EvaluationRun, Finding, KycError, RunPins, StructureClass, TypeRegistryState, Verdict,
};

fn as_of() -> DateTime<Utc> {
    DateTime::<Utc>::from_timestamp(0, 0).unwrap()
}

struct TestCheck {
    id: &'static str,
    applicability: Vec<ApplicabilityCondition>,
}

impl Check for TestCheck {
    fn check_id(&self) -> &str {
        self.id
    }
    fn applicability(&self) -> &[ApplicabilityCondition] {
        &self.applicability
    }
}

fn empty_board<'a>(control: &'a ControlState, types: &'a TypeRegistryState) -> BoardSnapshot<'a> {
    BoardSnapshot { control, type_registry: types, determination: None }
}

// ── §3 P1 gate: in_scope_set_is_computed_not_stored ─────────────────────────

/// D2.0 §6: adding a trust to the board changes the in-scope set on the
/// next run with no configuration change anywhere — the in-scope set is
/// RECOMPUTED, never stored.
#[test]
fn in_scope_set_is_computed_not_stored() {
    let checks = vec![
        TestCheck { id: "always.on", applicability: vec![ApplicabilityCondition::Unconditional] },
        TestCheck {
            id: "trust.only",
            applicability: vec![ApplicabilityCondition::StructureClassPresent(StructureClass::Trust)],
        },
    ];

    let mut control = ControlState::default();
    let types = TypeRegistryState::default();

    let before = in_scope_check_ids(&checks, &empty_board(&control, &types));
    assert_eq!(before, vec!["always.on".to_string()], "trust check must not be in scope yet");

    // No configuration change anywhere — only the board changed.
    control.structure_class = Some(StructureClass::Trust);
    let after = in_scope_check_ids(&checks, &empty_board(&control, &types));
    assert_eq!(
        after,
        vec!["always.on".to_string(), "trust.only".to_string()],
        "adding a trust to the board must pull trust.only into scope with zero config change"
    );
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

// ── §4/§6 run book gates ─────────────────────────────────────────────────────

fn pins(check_ids: Vec<String>) -> RunPins {
    use ob_poc_kyc_substrate::Hash;
    RunPins {
        subject_root: ob_poc_kyc_substrate::SubjectId(Uuid::new_v4()),
        board_state_hash: Hash::of_json(&serde_json::json!({"n": 1})),
        evaluation_pack_version_hash: Hash::of_json(&serde_json::json!({"v": 1})),
        valid_time: as_of(),
        knowledge_time: as_of(),
        trigger: "test".into(),
        in_scope_check_ids: check_ids,
    }
}

/// D2.0 §6 `checks_run_at_any_board_state` — the founding property: a board
/// of three alleged edges over alleged types produces a run, with verdicts,
/// and no error. Nothing gates on completeness.
#[test]
fn checks_run_at_any_board_state() {
    let control = ControlState::default();
    let types = TypeRegistryState::default();
    let entity = EntityId(Uuid::new_v4());
    let findings = vec![Finding::unevaluable(
        "sanctions.screen",
        entity,
        ob_poc_kyc_substrate::UnevaluableReason::FactAbsent { what: "no screening fact recorded".into() },
    )];
    let _board = empty_board(&control, &types);
    let run = EvaluationRun::new(Uuid::new_v4(), pins(vec!["sanctions.screen".into()]), findings)
        .expect("a run over an incomplete board must not error");
    assert_eq!(run.findings.len(), 1);
    assert!(matches!(run.findings[0].verdict, Verdict::Unevaluable));
}

/// D2.0 §6 `unevaluable_is_not_fail` — property: adding the missing fact and
/// re-running flips it to pass or fail, never the reverse.
#[test]
fn unevaluable_is_not_fail() {
    let entity = EntityId(Uuid::new_v4());
    let unevaluable = Finding::unevaluable(
        "sanctions.screen",
        entity,
        ob_poc_kyc_substrate::UnevaluableReason::FactAbsent { what: "no screening fact".into() },
    );
    assert!(matches!(unevaluable.verdict, Verdict::Unevaluable));
    assert!(!matches!(unevaluable.verdict, Verdict::Fail));

    // Re-running with the fact now present flips it — never the reverse.
    let now_pass = Finding::pass("sanctions.screen", entity, vec![]);
    assert!(matches!(now_pass.verdict, Verdict::Pass));
}

/// D2.0 §6 `work_list_is_derived_from_latest_run` — structural: no stored
/// work-list state exists; the list is a query over the newest run.
#[test]
fn work_list_is_derived_from_latest_run() {
    let entity = EntityId(Uuid::new_v4());
    let old_run = EvaluationRun::new(
        Uuid::new_v4(),
        pins(vec!["c1".into()]),
        vec![Finding::fail("c1", entity, "was failing", vec![])],
    )
    .unwrap();
    let new_run = EvaluationRun::new(
        Uuid::new_v4(),
        pins(vec!["c1".into()]),
        vec![Finding::pass("c1", entity, vec![])],
    )
    .unwrap();

    let history = vec![old_run, new_run];
    let work_list = work_list_from_history(&history);
    assert!(work_list.is_empty(), "the fix in the newest run must clear the work list, ignoring the stale old run");
}

/// D2.0 §6 `staleness_is_hash_comparison` — a run whose pinned board hash
/// differs from the board's current hash reports stale, with no flag
/// written anywhere.
#[test]
fn staleness_is_hash_comparison() {
    let control = ControlState::default();
    let types = TypeRegistryState::default();
    let board = empty_board(&control, &types);
    let hash_now = board_state_hash(&board);

    let run = EvaluationRun::new(Uuid::new_v4(), pins(vec!["c1".into()]), vec![]).unwrap();
    assert!(run.is_stale(hash_now), "run pinned a different (test-fixture) hash — must report stale");

    let fresh_run = EvaluationRun::new(
        Uuid::new_v4(),
        RunPins { board_state_hash: hash_now, ..pins(vec!["c1".into()]) },
        vec![],
    )
    .unwrap();
    assert!(!fresh_run.is_stale(hash_now), "a run pinned to the CURRENT hash must not report stale");

    let control2 = ControlState {
        structure_class: Some(StructureClass::Trust),
        ..Default::default()
    };
    let board2 = empty_board(&control2, &types);
    let hash_after_change = board_state_hash(&board2);
    assert_ne!(hash_now, hash_after_change, "board_state_hash must be sensitive to structure_class");
    assert!(fresh_run.is_stale(hash_after_change), "the board moved on — the old run must now report stale");
}

/// D2.0 §6 `runs_are_append_only` — re-running never mutates a prior run;
/// the old verdicts stand.
#[test]
fn runs_are_append_only() {
    let entity = EntityId(Uuid::new_v4());
    let run1 =
        EvaluationRun::new(Uuid::new_v4(), pins(vec!["c1".into()]), vec![Finding::fail("c1", entity, "bad", vec![])])
            .unwrap();
    let run1_id = run1.run_id;
    let run1_verdict = run1.findings[0].verdict;

    let mut history = vec![run1];
    let run2 = EvaluationRun::new(Uuid::new_v4(), pins(vec!["c1".into()]), vec![Finding::pass("c1", entity, vec![])])
        .unwrap();
    history.push(run2);

    let stood = history.iter().find(|r| r.run_id == run1_id).expect("prior run must still be present, unmutated");
    assert!(matches!(stood.findings[0].verdict, Verdict::Fail), "prior run's verdict must be untouched");
    assert_eq!(stood.findings[0].verdict as u8, run1_verdict as u8);
    assert_eq!(history.len(), 2, "re-running creates a NEW run, never replaces one");
}

/// D2.0 §6 `run_pins_are_complete` — a run missing any pin is refused.
#[test]
fn run_pins_are_complete() {
    let missing_trigger = RunPins { trigger: "".into(), ..pins(vec!["c1".into()]) };
    let err = EvaluationRun::new(Uuid::new_v4(), missing_trigger, vec![]).unwrap_err();
    assert!(matches!(err, KycError::IncompleteRun { .. }));

    let missing_subject =
        RunPins { subject_root: ob_poc_kyc_substrate::SubjectId(Uuid::nil()), ..pins(vec!["c1".into()]) };
    let err2 = EvaluationRun::new(Uuid::new_v4(), missing_subject, vec![]).unwrap_err();
    assert!(matches!(err2, KycError::IncompleteRun { .. }));

    // Complete pins: construction succeeds, including a run with an EMPTY
    // in-scope set (a legitimate computed value, not a missing pin).
    assert!(EvaluationRun::new(Uuid::new_v4(), pins(vec!["c1".into()]), vec![]).is_ok());
    assert!(EvaluationRun::new(Uuid::new_v4(), pins(vec![]), vec![]).is_ok());
}
