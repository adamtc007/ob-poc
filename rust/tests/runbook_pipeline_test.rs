//! Runbook Pipeline Integration Tests
//!
//! External test harness — uses only the crate's public API.
//! Macro registries loaded from YAML fixtures.

use std::collections::{BTreeMap, HashSet};
use uuid::Uuid;

use ob_poc::dsl_v2::{load_macro_registry_from_dir, MacroRegistry};
use ob_poc::journey::pack_manager::{ConstraintSource, EffectiveConstraints};
use ob_poc::repl::verb_config_index::VerbConfigIndex;
use ob_poc::runbook::{
    classify_verb, compile_verb, compute_write_set, execute_runbook, CompiledRunbook,
    CompiledRunbookStatus, CompiledStep, ExecutionError, ExecutionMode, OrchestratorResponse,
    ParkReason, ReplayEnvelope, RunbookExecutionResult, RunbookStore, RunbookStoreBackend,
    StepCursor, StepExecutor, StepOutcome, VerbClassification,
};
use ob_poc::session::unified::{ClientRef, StructureType, UnifiedSession};

// =============================================================================
// Fixture helpers
// =============================================================================

fn fixture_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/macros")
}

fn load_fixture_registry() -> MacroRegistry {
    load_macro_registry_from_dir(&fixture_dir())
        .expect("fixture macros must load")
}

fn test_session() -> UnifiedSession {
    UnifiedSession {
        client: Some(ClientRef {
            client_id: Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
            display_name: "Test Client".to_string(),
        }),
        structure_type: Some(StructureType::Pe),
        ..Default::default()
    }
}

fn make_step(verb: &str) -> CompiledStep {
    CompiledStep {
        step_id: Uuid::new_v4(),
        sentence: format!("Do {verb}"),
        verb: verb.into(),
        dsl: format!("({verb})"),
        args: BTreeMap::new(),
        depends_on: vec![],
        execution_mode: ExecutionMode::Sync,
        write_set: vec![],
        verb_contract_snapshot_id: None,
    }
}

struct SuccessExecutor;
#[async_trait::async_trait]
impl StepExecutor for SuccessExecutor {
    async fn execute_step(&self, _step: &CompiledStep) -> StepOutcome {
        StepOutcome::Completed {
            result: serde_json::json!({"ok": true}),
        }
    }
}

struct FailOnVerb(String);
#[async_trait::async_trait]
impl StepExecutor for FailOnVerb {
    async fn execute_step(&self, step: &CompiledStep) -> StepOutcome {
        if step.verb == self.0 {
            StepOutcome::Failed { error: format!("{} failed", step.verb) }
        } else {
            StepOutcome::Completed { result: serde_json::json!({"ok": true}) }
        }
    }
}

struct ParkOnVerb(String);
#[async_trait::async_trait]
impl StepExecutor for ParkOnVerb {
    async fn execute_step(&self, step: &CompiledStep) -> StepOutcome {
        if step.verb == self.0 {
            StepOutcome::Parked {
                correlation_key: format!("park-{}", step.step_id),
                message: "Waiting for approval".into(),
            }
        } else {
            StepOutcome::Completed { result: serde_json::json!({"ok": true}) }
        }
    }
}

struct RecordingExecutor {
    executed: std::sync::Mutex<Vec<String>>,
}
impl RecordingExecutor {
    fn new() -> Self { Self { executed: std::sync::Mutex::new(Vec::new()) } }
    fn executed_verbs(&self) -> Vec<String> { self.executed.lock().unwrap().clone() }
}
#[async_trait::async_trait]
impl StepExecutor for RecordingExecutor {
    async fn execute_step(&self, step: &CompiledStep) -> StepOutcome {
        self.executed.lock().unwrap().push(step.verb.clone());
        StepOutcome::Completed { result: serde_json::json!({"ok": true}) }
    }
}

async fn compile_and_execute(
    macro_name: &str,
    args: BTreeMap<String, String>,
    registry: &MacroRegistry,
    constraints: &EffectiveConstraints,
) -> (OrchestratorResponse, Option<RunbookExecutionResult>) {
    let session = test_session();
    let verb_index = VerbConfigIndex::empty();
    let classification = classify_verb(macro_name, &verb_index, registry);

    let resp = compile_verb(
        Uuid::new_v4(), &classification, &args, &session, registry,
        1, constraints, None, None,
    );

    if let OrchestratorResponse::Compiled(ref summary) = resp {
        let steps: Vec<_> = summary.preview.iter().map(|p| make_step(&p.verb)).collect();
        let rb = CompiledRunbook::new(Uuid::new_v4(), 1, steps, ReplayEnvelope::empty());
        let id = rb.id;
        let store = RunbookStore::new();
        store.insert(&rb).await.unwrap();
        let result = execute_runbook(&store, id, None, &SuccessExecutor).await.unwrap();
        (resp, Some(result))
    } else {
        (resp, None)
    }
}

// =============================================================================
// Test 1: macro_end_to_end (§8.1, §11.1)
// =============================================================================

#[tokio::test]
async fn macro_end_to_end() {
    let registry = load_fixture_registry();
    let constraints = EffectiveConstraints::unconstrained();

    let mut args = BTreeMap::new();
    args.insert("name".to_string(), "Acme Fund".to_string());

    let (resp, exec_result) =
        compile_and_execute("structure.setup", args, &registry, &constraints).await;

    assert!(resp.is_compiled(), "Expected Compiled, got {:?}", resp);
    let summary = match &resp {
        OrchestratorResponse::Compiled(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(summary.step_count, 1);
    assert_eq!(summary.preview[0].verb, "cbu.create");

    let result = exec_result.unwrap();
    assert!(matches!(result.final_status, CompiledRunbookStatus::Completed { .. }));
    assert_eq!(result.step_results.len(), 1);
}

// =============================================================================
// Test 2: pack_scoping (§6, §7.3, §8.2)
// =============================================================================

#[tokio::test]
async fn pack_scoping() {
    let registry = load_fixture_registry();
    let mut allowed = HashSet::new();
    allowed.insert("kyc.create-case".to_string());
    allowed.insert("kyc.assign-analyst".to_string());
    let constraints = EffectiveConstraints {
        allowed_verbs: Some(allowed),
        forbidden_verbs: HashSet::new(),
        contributing_packs: vec![ConstraintSource {
            pack_id: "kyc-case".to_string(),
            pack_name: "KYC Case".to_string(),
            allowed_count: 2,
            forbidden_count: 0,
        }],
    };

    let mut args = BTreeMap::new();
    args.insert("name".to_string(), "Acme Fund".to_string());

    let (resp, exec_result) =
        compile_and_execute("structure.setup", args, &registry, &constraints).await;

    assert!(matches!(resp, OrchestratorResponse::ConstraintViolation(_)));
    assert!(exec_result.is_none());
}

// =============================================================================
// Test 3: pack_completion_widening (§6.3)
// =============================================================================

#[tokio::test]
async fn pack_completion_widening() {
    use ob_poc::journey::pack::PackManifest;
    use ob_poc::journey::pack_manager::PackManager;

    let mut manager = PackManager::new();
    let manifest = PackManifest {
        id: "kyc-case".to_string(),
        name: "KYC Case".to_string(),
        version: "1.0".to_string(),
        description: "KYC case management".to_string(),
        invocation_phrases: vec![],
        required_context: vec![],
        optional_context: vec![],
        allowed_verbs: vec!["kyc.create-case".to_string()],
        forbidden_verbs: vec![],
        risk_policy: Default::default(),
        required_questions: vec![],
        optional_questions: vec![],
        stop_rules: vec![],
        templates: vec![],
        pack_summary_template: None,
        section_layout: vec![],
        definition_of_done: vec![],
        progress_signals: vec![],
        handoff_target: None,
        workspaces: vec![ob_poc::repl::types_v2::WorkspaceKind::OnBoarding],
    };
    manager.register_pack(manifest);
    manager.activate_pack("kyc-case").unwrap();

    let before = manager.effective_constraints();
    assert!(!before.allowed_verbs.as_ref().unwrap().contains("cbu.create"));

    manager.complete_pack("kyc-case").unwrap();
    let after = manager.effective_constraints();
    assert!(after.allowed_verbs.is_none(), "After completion, unconstrained");

    let registry = load_fixture_registry();
    let mut args = BTreeMap::new();
    args.insert("name".to_string(), "Widened Fund".to_string());

    let (resp, exec_result) =
        compile_and_execute("structure.setup", args, &registry, &after).await;

    assert!(resp.is_compiled());
    assert!(matches!(exec_result.unwrap().final_status, CompiledRunbookStatus::Completed { .. }));
}

// =============================================================================
// Test 4: replay_determinism (§9, INV-2)
// =============================================================================

#[tokio::test]
async fn replay_determinism() {
    let registry = load_fixture_registry();
    let session = test_session();
    let verb_index = VerbConfigIndex::empty();
    let constraints = EffectiveConstraints::unconstrained();

    let mut args = BTreeMap::new();
    args.insert("name".to_string(), "Determinism Test".to_string());

    let classification = classify_verb("structure.full-setup", &verb_index, &registry);

    let resp1 = compile_verb(Uuid::new_v4(), &classification, &args, &session, &registry, 1, &constraints, None, None);
    let resp2 = compile_verb(Uuid::new_v4(), &classification, &args, &session, &registry, 1, &constraints, None, None);

    let s1 = match resp1 { OrchestratorResponse::Compiled(s) => s, _ => panic!("compile 1 failed") };
    let s2 = match resp2 { OrchestratorResponse::Compiled(s) => s, _ => panic!("compile 2 failed") };

    assert_eq!(s1.step_count, s2.step_count);
    for (a, b) in s1.preview.iter().zip(s2.preview.iter()) {
        assert_eq!(a.verb, b.verb, "Verb order must be deterministic");
    }

    let store1 = RunbookStore::new();
    let steps1: Vec<_> = s1.preview.iter().map(|p| make_step(&p.verb)).collect();
    let rb1 = CompiledRunbook::new(Uuid::new_v4(), 1, steps1, ReplayEnvelope::empty());
    let id1 = rb1.id;
    store1.insert(&rb1).await.unwrap();

    let store2 = RunbookStore::new();
    let steps2: Vec<_> = s2.preview.iter().map(|p| make_step(&p.verb)).collect();
    let rb2 = CompiledRunbook::new(Uuid::new_v4(), 1, steps2, ReplayEnvelope::empty());
    let id2 = rb2.id;
    store2.insert(&rb2).await.unwrap();

    let r1 = RecordingExecutor::new();
    let r2 = RecordingExecutor::new();
    let _ = execute_runbook(&store1, id1, None, &r1).await.unwrap();
    let _ = execute_runbook(&store2, id2, None, &r2).await.unwrap();
    assert_eq!(r1.executed_verbs(), r2.executed_verbs());
}

// =============================================================================
// Test 5: locking_no_deadlock (§10.2)
// =============================================================================

#[tokio::test]
async fn locking_no_deadlock() {
    let id_a = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
    let id_b = Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap();
    let id_c = Uuid::parse_str("cccccccc-cccc-cccc-cccc-cccccccccccc").unwrap();

    let mut s1 = make_step("cbu.create"); s1.write_set = vec![id_c, id_a];
    let mut s2 = make_step("entity.create"); s2.write_set = vec![id_b, id_a];
    let mut s3 = make_step("cbu.rename"); s3.write_set = vec![id_b, id_c];
    let mut s4 = make_step("entity.update"); s4.write_set = vec![id_a];

    let ws1 = compute_write_set(&[s1, s2], None);
    let ws2 = compute_write_set(&[s3, s4], None);

    let order1: Vec<Uuid> = ws1.into_iter().collect();
    let order2: Vec<Uuid> = ws2.into_iter().collect();
    assert_eq!(order1, order2, "Lock order must be deterministic");
}

// =============================================================================
// Test 6: execution_gate_rejects_raw (§11.1, INV-3)
// =============================================================================

#[tokio::test]
async fn execution_gate_rejects_raw() {
    let store = RunbookStore::new();

    let fake_rb = CompiledRunbook::new(Uuid::new_v4(), 1, vec![], ReplayEnvelope::empty());
    let fake_id = fake_rb.id;
    assert!(matches!(execute_runbook(&store, fake_id, None, &SuccessExecutor).await, Err(ExecutionError::NotFound(_))));

    let rb = CompiledRunbook::new(Uuid::new_v4(), 1, vec![make_step("cbu.create")], ReplayEnvelope::empty());
    let id = rb.id;
    store.insert(&rb).await.unwrap();
    let result = execute_runbook(&store, id, None, &SuccessExecutor).await.unwrap();
    assert!(matches!(result.final_status, CompiledRunbookStatus::Completed { .. }));

    assert!(matches!(execute_runbook(&store, id, None, &SuccessExecutor).await, Err(ExecutionError::NotExecutable(_, _))));

    let rb2 = CompiledRunbook::new(Uuid::new_v4(), 2, vec![make_step("entity.create")], ReplayEnvelope::empty());
    let id2 = rb2.id;
    store.insert(&rb2).await.unwrap();
    let result = execute_runbook(&store, id2, None, &FailOnVerb("entity.create".into())).await.unwrap();
    assert!(matches!(result.final_status, CompiledRunbookStatus::Failed { .. }));
    assert!(matches!(execute_runbook(&store, id2, None, &SuccessExecutor).await, Err(ExecutionError::NotExecutable(_, _))));
}

// =============================================================================
// Test 7: constraint_violation_remediation (§11.3)
// =============================================================================

#[tokio::test]
async fn constraint_violation_remediation() {
    let registry = load_fixture_registry();
    let mut allowed = HashSet::new();
    allowed.insert("session.load-galaxy".to_string());
    allowed.insert("session.load-cbu".to_string());
    let constraints = EffectiveConstraints {
        allowed_verbs: Some(allowed),
        forbidden_verbs: HashSet::new(),
        contributing_packs: vec![ConstraintSource {
            pack_id: "session-bootstrap".to_string(),
            pack_name: "Session Bootstrap".to_string(),
            allowed_count: 2,
            forbidden_count: 0,
        }],
    };

    let session = test_session();
    let verb_index = VerbConfigIndex::empty();
    let classification = classify_verb("structure.setup", &verb_index, &registry);

    let mut args = BTreeMap::new();
    args.insert("name".to_string(), "Blocked Fund".to_string());

    let resp = compile_verb(Uuid::new_v4(), &classification, &args, &session, &registry, 1, &constraints, None, None);

    let detail = match resp {
        OrchestratorResponse::ConstraintViolation(d) => d,
        other => panic!("Expected ConstraintViolation, got {:?}", other),
    };
    assert!(!detail.violating_verbs.is_empty());
    assert!(detail.violating_verbs.contains(&"cbu.create".to_string()));
    assert!(!detail.active_constraints.is_empty());
    assert!(!detail.remediation_options.is_empty());
}

// =============================================================================
// Test 8: runbook_immutability (§1, INV-1a)
// =============================================================================

#[tokio::test]
async fn runbook_immutability() {
    let store = RunbookStore::new();

    let step1 = make_step("cbu.create");
    let step2 = make_step("entity.create");
    let step3 = make_step("trading-profile.create");
    let original_ids: Vec<Uuid> = vec![step1.step_id, step2.step_id, step3.step_id];

    let rb = CompiledRunbook::new(Uuid::new_v4(), 1, vec![step1, step2, step3], ReplayEnvelope::empty());
    let id = rb.id;
    let version = rb.version;
    let session_id = rb.session_id;
    store.insert(&rb).await.unwrap();

    let before = store.get(&id).await.unwrap().unwrap();
    assert_eq!(before.steps.len(), 3);

    let result = execute_runbook(&store, id, None, &SuccessExecutor).await.unwrap();
    assert!(matches!(result.final_status, CompiledRunbookStatus::Completed { .. }));

    let after = store.get(&id).await.unwrap().unwrap();
    assert_eq!(after.steps.len(), 3);
    for (i, step) in after.steps.iter().enumerate() {
        assert_eq!(step.step_id, original_ids[i]);
    }
    assert_eq!(after.version, version);
    assert_eq!(after.session_id, session_id);
}

// =============================================================================
// Test 9: multi_step_macro_pipeline
// =============================================================================

#[tokio::test]
async fn multi_step_macro_pipeline() {
    let registry = load_fixture_registry();
    let constraints = EffectiveConstraints::unconstrained();

    let mut args = BTreeMap::new();
    args.insert("name".to_string(), "Multi-Step Fund".to_string());

    let (resp, exec_result) =
        compile_and_execute("structure.full-setup", args, &registry, &constraints).await;

    assert!(resp.is_compiled());
    let summary = match &resp { OrchestratorResponse::Compiled(s) => s, _ => unreachable!() };
    assert_eq!(summary.step_count, 2);

    assert!(matches!(exec_result.unwrap().final_status, CompiledRunbookStatus::Completed { .. }));
}

// =============================================================================
// Test 10: park_and_resume_pipeline
// =============================================================================

#[tokio::test]
async fn park_and_resume_pipeline() {
    let store = RunbookStore::new();
    let step1 = make_step("cbu.create");
    let step2 = make_step("doc.solicit");
    let step2_id = step2.step_id;
    let step3 = make_step("entity.create");

    let rb = CompiledRunbook::new(Uuid::new_v4(), 1, vec![step1, step2, step3], ReplayEnvelope::empty());
    let id = rb.id;
    store.insert(&rb).await.unwrap();

    let result = execute_runbook(&store, id, None, &ParkOnVerb("doc.solicit".into())).await.unwrap();
    assert!(matches!(result.final_status, CompiledRunbookStatus::Parked { .. }));

    store.update_status(&id, "Parked", CompiledRunbookStatus::Parked {
        reason: ParkReason::AwaitingCallback { correlation_key: "callback-1".into() },
        cursor: StepCursor { index: 1, step_id: step2_id },
    }).await.unwrap();

    let cursor = StepCursor { index: 1, step_id: step2_id };
    let result = execute_runbook(&store, id, Some(cursor), &SuccessExecutor).await.unwrap();
    assert!(matches!(result.final_status, CompiledRunbookStatus::Completed { .. }));
}

// =============================================================================
// Test 11: primitive_verb_compile_and_execute
// =============================================================================

#[tokio::test]
async fn primitive_verb_compile_and_execute() {
    let registry = MacroRegistry::new();
    let session = test_session();
    let constraints = EffectiveConstraints::unconstrained();

    let classification = VerbClassification::Primitive { fqn: "cbu.create".to_string() };
    let mut args = BTreeMap::new();
    args.insert("name".to_string(), "Direct Fund".to_string());

    let resp = compile_verb(Uuid::new_v4(), &classification, &args, &session, &registry, 1, &constraints, None, None);
    assert!(resp.is_compiled());

    let summary = match &resp { OrchestratorResponse::Compiled(s) => s, _ => unreachable!() };
    let store = RunbookStore::new();
    let steps: Vec<_> = summary.preview.iter().map(|p| make_step(&p.verb)).collect();
    let rb = CompiledRunbook::new(Uuid::new_v4(), 1, steps, ReplayEnvelope::empty());
    let id = rb.id;
    store.insert(&rb).await.unwrap();
    let result = execute_runbook(&store, id, None, &SuccessExecutor).await.unwrap();
    assert!(matches!(result.final_status, CompiledRunbookStatus::Completed { .. }));
}

// =============================================================================
// Test 12-14: compile_invocation tests
// =============================================================================

#[test]
fn compile_invocation_missing_macro_args() {
    let registry = load_fixture_registry();
    let verb_index = VerbConfigIndex::empty();
    let session = test_session();
    let constraints = EffectiveConstraints::unconstrained();
    let args = BTreeMap::new();

    let resp = ob_poc::runbook::compile_invocation(
        Uuid::new_v4(), "structure.setup", &args, &session, &registry,
        &verb_index, &constraints, 1, None, None,
    );

    assert!(matches!(resp, OrchestratorResponse::Clarification(_)));
    if let OrchestratorResponse::Clarification(c) = &resp {
        let names: Vec<&str> = c.missing_fields.iter().map(|f| f.field_name.as_str()).collect();
        assert!(names.contains(&"name"));
    }
}

#[test]
fn compile_invocation_end_to_end() {
    let registry = load_fixture_registry();
    let verb_index = VerbConfigIndex::empty();
    let session = test_session();
    let constraints = EffectiveConstraints::unconstrained();
    let mut args = BTreeMap::new();
    args.insert("name".to_string(), "E2E Fund".to_string());

    let resp = ob_poc::runbook::compile_invocation(
        Uuid::new_v4(), "structure.setup", &args, &session, &registry,
        &verb_index, &constraints, 1, None, None,
    );

    assert!(resp.is_compiled());
    let summary = match &resp { OrchestratorResponse::Compiled(s) => s, _ => unreachable!() };
    assert_eq!(summary.preview[0].verb, "cbu.create");
}

#[test]
fn compile_invocation_constraint_violation() {
    let registry = load_fixture_registry();
    let verb_index = VerbConfigIndex::empty();
    let session = test_session();
    let mut allowed = HashSet::new();
    allowed.insert("kyc.create-case".to_string());
    let constraints = EffectiveConstraints {
        allowed_verbs: Some(allowed),
        forbidden_verbs: HashSet::new(),
        contributing_packs: vec![ConstraintSource {
            pack_id: "kyc-case".to_string(),
            pack_name: "KYC Case".to_string(),
            allowed_count: 1,
            forbidden_count: 0,
        }],
    };
    let mut args = BTreeMap::new();
    args.insert("name".to_string(), "Blocked Fund".to_string());

    let resp = ob_poc::runbook::compile_invocation(
        Uuid::new_v4(), "structure.setup", &args, &session, &registry,
        &verb_index, &constraints, 1, None, None,
    );

    assert!(matches!(resp, OrchestratorResponse::ConstraintViolation(_)));
}
