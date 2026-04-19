//! End-to-End Acceptance Tests for Runbook Compilation + Execution
//!
//! External test harness — uses only the crate's public API.
//! Macro registries are loaded from YAML fixtures (no internal type construction).
//!
//! Tests that exercise macro expansion internals (cycle detection, depth limits,
//! nested fixpoint) live inside the crate at `dsl_v2::macros` (#[cfg(test)]).
//! Source-scanning invariant tests live at `runbook::invariant_tests`.

use std::collections::{BTreeMap, HashSet};
use uuid::Uuid;

use ob_poc::dsl_v2::{load_macro_registry_from_dir, MacroRegistry};
use ob_poc::journey::pack_manager::{ConstraintSource, EffectiveConstraints};
use ob_poc::repl::verb_config_index::VerbConfigIndex;
use ob_poc::runbook::{
    canonical_bytes_for_steps, classify_verb, compile_invocation, compile_verb, compute_write_set,
    content_addressed_id, derive_write_set_heuristic, execute_runbook, full_sha256,
    CompiledRunbook, CompiledRunbookStatus, CompiledStep, ExecutionError, ExecutionMode,
    OrchestratorResponse, ReplayEnvelope, RunbookStore, RunbookStoreBackend, StepExecutor,
    StepOutcome, VerbClassification,
};
use ob_poc::session::unified::{ClientRef, StructureType, UnifiedSession};

// =============================================================================
// Fixture helpers
// =============================================================================

fn fixture_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/macros")
}

fn load_structure_registry() -> MacroRegistry {
    load_macro_registry_from_dir(&fixture_dir()).expect("fixture macros must load")
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
        sentence: format!("Execute {verb}"),
        verb: verb.into(),
        dsl: format!("({verb})"),
        args: BTreeMap::new(),
        depends_on: vec![],
        execution_mode: ExecutionMode::Sync,
        write_set: vec![],
        verb_contract_snapshot_id: None,
    }
}

fn make_step_with_write_set(verb: &str, write_set: Vec<Uuid>) -> CompiledStep {
    CompiledStep {
        step_id: Uuid::new_v4(),
        sentence: format!("Execute {verb}"),
        verb: verb.into(),
        dsl: format!("({verb})"),
        args: BTreeMap::new(),
        depends_on: vec![],
        execution_mode: ExecutionMode::Sync,
        write_set,
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

struct RecordingExecutor {
    executed: std::sync::Mutex<Vec<(String, Vec<Uuid>)>>,
}

impl RecordingExecutor {
    fn new() -> Self {
        Self {
            executed: std::sync::Mutex::new(Vec::new()),
        }
    }
    fn executed_verbs(&self) -> Vec<String> {
        self.executed
            .lock()
            .unwrap()
            .iter()
            .map(|(v, _)| v.clone())
            .collect()
    }
}

#[async_trait::async_trait]
impl StepExecutor for RecordingExecutor {
    async fn execute_step(&self, step: &CompiledStep) -> StepOutcome {
        self.executed
            .lock()
            .unwrap()
            .push((step.verb.clone(), step.write_set.clone()));
        StepOutcome::Completed {
            result: serde_json::json!({"ok": true}),
        }
    }
}

// =============================================================================
// Test 1: Primitive verb round-trip (INV-1, INV-9)
// =============================================================================

#[tokio::test]
async fn test_primitive_verb_round_trip() {
    let registry = MacroRegistry::new();
    let session = test_session();
    let constraints = EffectiveConstraints::unconstrained();

    let classification = VerbClassification::Primitive {
        fqn: "cbu.create".to_string(),
    };

    let mut args = BTreeMap::new();
    args.insert("name".to_string(), "E2E Fund".to_string());

    let resp = compile_verb(
        Uuid::new_v4(),
        &classification,
        &args,
        &session,
        &registry,
        1,
        &constraints,
        None,
        None,
    );

    assert!(
        resp.is_compiled(),
        "Primitive verb must compile: {:?}",
        resp
    );
    let summary = match &resp {
        OrchestratorResponse::Compiled(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(summary.step_count, 1);
    assert_eq!(summary.preview[0].verb, "cbu.create");

    let store = RunbookStore::new();
    let steps: Vec<_> = summary.preview.iter().map(|p| make_step(&p.verb)).collect();
    let rb = CompiledRunbook::new(Uuid::new_v4(), 1, steps, ReplayEnvelope::empty());
    let id = rb.id;
    store.insert(&rb).await.unwrap();

    let retrieved = store
        .get(&id)
        .await
        .unwrap()
        .expect("Must retrieve stored runbook");
    assert_eq!(retrieved.steps.len(), 1);
    assert!(matches!(retrieved.status, CompiledRunbookStatus::Compiled));

    let result = execute_runbook(&store, id, None, &SuccessExecutor)
        .await
        .unwrap();
    assert!(matches!(
        result.final_status,
        CompiledRunbookStatus::Completed { .. }
    ));
    assert_eq!(result.step_results.len(), 1);

    let after = store.get(&id).await.unwrap().unwrap();
    assert!(matches!(
        after.status,
        CompiledRunbookStatus::Completed { .. }
    ));
}

// =============================================================================
// Test 2: Macro compiles and executes through pipeline (INV-4)
// =============================================================================

#[tokio::test]
async fn test_macro_compiles_and_executes() {
    let registry = load_structure_registry();
    let session = test_session();
    let verb_index = VerbConfigIndex::empty();
    let constraints = EffectiveConstraints::unconstrained();
    let classification = classify_verb("structure.setup", &verb_index, &registry);

    let mut args = BTreeMap::new();
    args.insert("name".to_string(), "Macro Test Fund".to_string());

    let resp = compile_verb(
        Uuid::new_v4(),
        &classification,
        &args,
        &session,
        &registry,
        1,
        &constraints,
        None,
        None,
    );

    assert!(resp.is_compiled(), "Macro must compile: {:?}", resp);
    let summary = match &resp {
        OrchestratorResponse::Compiled(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(summary.preview[0].verb, "cbu.create");

    let store = RunbookStore::new();
    let steps: Vec<_> = summary.preview.iter().map(|p| make_step(&p.verb)).collect();
    let rb = CompiledRunbook::new(Uuid::new_v4(), 1, steps, ReplayEnvelope::empty());
    let id = rb.id;
    store.insert(&rb).await.unwrap();

    let result = execute_runbook(&store, id, None, &SuccessExecutor)
        .await
        .unwrap();
    assert!(matches!(
        result.final_status,
        CompiledRunbookStatus::Completed { .. }
    ));
}

// =============================================================================
// Test 3: Pack constraint blocks forbidden verb (INV-6)
// =============================================================================

#[test]
fn test_pack_constraint_blocks_forbidden_verb() {
    let registry = load_structure_registry();
    let verb_index = VerbConfigIndex::empty();
    let session = test_session();

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
    args.insert("name".to_string(), "Blocked Fund".to_string());

    let resp = compile_invocation(
        Uuid::new_v4(),
        "structure.setup",
        &args,
        &session,
        &registry,
        &verb_index,
        &constraints,
        1,
        None,
        None,
    );

    assert!(
        matches!(resp, OrchestratorResponse::ConstraintViolation(_)),
        "Expanded verb outside pack must be blocked (INV-6), got: {:?}",
        resp
    );

    if let OrchestratorResponse::ConstraintViolation(detail) = &resp {
        assert!(detail.violating_verbs.contains(&"cbu.create".to_string()));
        assert!(!detail.active_constraints.is_empty());
        assert!(!detail.remediation_options.is_empty());
    }
}

// =============================================================================
// Test 4: Content-addressed ID determinism (INV-2, INV-13)
// =============================================================================

#[test]
fn test_content_addressed_id_determinism() {
    let env = ReplayEnvelope::empty();
    let step_id = Uuid::nil();

    let step_a = CompiledStep {
        step_id,
        sentence: "Execute cbu.create".into(),
        verb: "cbu.create".into(),
        dsl: "(cbu.create)".into(),
        args: {
            let mut m = BTreeMap::new();
            m.insert("name".to_string(), "Acme".to_string());
            m
        },
        depends_on: vec![],
        execution_mode: ExecutionMode::Sync,
        write_set: vec![],
        verb_contract_snapshot_id: None,
    };
    let step_b = step_a.clone();

    let id_a = content_addressed_id(std::slice::from_ref(&step_a), &env);
    let id_b = content_addressed_id(&[step_b], &env);
    assert_eq!(id_a, id_b, "INV-2: Same inputs must produce identical IDs");

    let step_different = CompiledStep {
        step_id,
        args: {
            let mut m = BTreeMap::new();
            m.insert("name".to_string(), "Beta Corp".to_string());
            m
        },
        ..step_a.clone()
    };
    let id_c = content_addressed_id(&[step_different], &env);
    assert_ne!(id_a, id_c, "Different args must produce different IDs");

    let two_steps = vec![
        step_a.clone(),
        CompiledStep {
            step_id: Uuid::nil(),
            sentence: "Execute entity.create".into(),
            verb: "entity.create".into(),
            dsl: "(entity.create)".into(),
            args: BTreeMap::new(),
            depends_on: vec![],
            execution_mode: ExecutionMode::Sync,
            write_set: vec![],
            verb_contract_snapshot_id: None,
        },
    ];
    let id_d = content_addressed_id(&two_steps, &env);
    assert_ne!(
        id_a, id_d,
        "INV-13: Different step counts must produce different IDs"
    );

    let hash = full_sha256(std::slice::from_ref(&step_a), &env);
    assert_eq!(hash.len(), 32);
    let expected_uuid = Uuid::from_bytes(hash[..16].try_into().unwrap());
    assert_eq!(id_a.0, expected_uuid);

    // BTreeMap ordering determinism (INV-2)
    let step_fwd = CompiledStep {
        step_id,
        args: {
            let mut m = BTreeMap::new();
            m.insert("alpha".to_string(), "1".to_string());
            m.insert("zebra".to_string(), "2".to_string());
            m
        },
        ..step_a.clone()
    };
    let step_rev = CompiledStep {
        step_id,
        args: {
            let mut m = BTreeMap::new();
            m.insert("zebra".to_string(), "2".to_string());
            m.insert("alpha".to_string(), "1".to_string());
            m
        },
        ..step_a.clone()
    };
    assert_eq!(
        canonical_bytes_for_steps(&[step_fwd]),
        canonical_bytes_for_steps(&[step_rev]),
        "INV-2: BTreeMap insertion order must not affect canonical bytes"
    );
}

// =============================================================================
// Test 5: Write set computation (INV-8)
// =============================================================================

#[tokio::test]
async fn test_write_set_computation() {
    let entity_id_1 = Uuid::new_v4();
    let entity_id_2 = Uuid::new_v4();

    let step1 = make_step_with_write_set("cbu.rename", vec![entity_id_1]);
    let step2 = make_step_with_write_set("entity.update", vec![entity_id_2, entity_id_1]);

    let ws = compute_write_set(&[step1.clone(), step2.clone()], None);
    assert!(ws.contains(&entity_id_1));
    assert!(ws.contains(&entity_id_2));
    assert_eq!(ws.len(), 2, "Duplicates must be removed");

    let sorted: Vec<Uuid> = ws.into_iter().collect();
    assert!(
        sorted.windows(2).all(|w| w[0] <= w[1]),
        "Must be sorted for deadlock prevention"
    );

    let store = RunbookStore::new();
    let rb = CompiledRunbook::new(
        Uuid::new_v4(),
        1,
        vec![step1, step2],
        ReplayEnvelope::empty(),
    );
    let id = rb.id;
    store.insert(&rb).await.unwrap();

    let recorder = RecordingExecutor::new();
    let result = execute_runbook(&store, id, None, &recorder).await.unwrap();
    assert!(matches!(
        result.final_status,
        CompiledRunbookStatus::Completed { .. }
    ));
    assert_eq!(
        recorder.executed_verbs(),
        vec!["cbu.rename", "entity.update"]
    );

    let mut heuristic_args = BTreeMap::new();
    heuristic_args.insert("entity-id".to_string(), entity_id_1.to_string());
    heuristic_args.insert("name".to_string(), "Acme Corp".to_string());
    let heuristic_ws = derive_write_set_heuristic(&heuristic_args);
    assert!(heuristic_ws.contains(&entity_id_1));
    assert_eq!(heuristic_ws.len(), 1, "Non-UUID args must be ignored");
}

// =============================================================================
// Test 6: Concurrent lock contention detection (INV-10)
// =============================================================================

#[tokio::test]
async fn test_concurrent_lock_contention() {
    let contested = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
    let entity_b = Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap();

    let rb1_step1 = make_step_with_write_set("cbu.rename", vec![contested]);
    let rb1_step2 = make_step_with_write_set("entity.update", vec![entity_b]);
    let rb2_step1 = make_step_with_write_set("trading-profile.update", vec![contested]);

    let ws1 = compute_write_set(&[rb1_step1.clone(), rb1_step2.clone()], None);
    let ws2 = compute_write_set(std::slice::from_ref(&rb2_step1), None);

    let overlap: Vec<Uuid> = ws1.intersection(&ws2).copied().collect();
    assert!(
        !overlap.is_empty(),
        "INV-10: Overlapping write_sets must be detectable"
    );
    assert_eq!(overlap[0], contested);

    // Both execute independently (no Postgres → no actual contention)
    let store1 = RunbookStore::new();
    let rb1 = CompiledRunbook::new(
        Uuid::new_v4(),
        1,
        vec![rb1_step1, rb1_step2],
        ReplayEnvelope::empty(),
    );
    let id1 = rb1.id;
    store1.insert(&rb1).await.unwrap();

    let store2 = RunbookStore::new();
    let rb2 = CompiledRunbook::new(Uuid::new_v4(), 1, vec![rb2_step1], ReplayEnvelope::empty());
    let id2 = rb2.id;
    store2.insert(&rb2).await.unwrap();

    let r1 = execute_runbook(&store1, id1, None, &SuccessExecutor)
        .await
        .unwrap();
    let r2 = execute_runbook(&store2, id2, None, &SuccessExecutor)
        .await
        .unwrap();
    assert!(matches!(
        r1.final_status,
        CompiledRunbookStatus::Completed { .. }
    ));
    assert!(matches!(
        r2.final_status,
        CompiledRunbookStatus::Completed { .. }
    ));
}

// =============================================================================
// Test 7: No execution without compiled ID (INV-1)
// =============================================================================

#[tokio::test]
async fn test_no_execution_without_compiled_id() {
    let store = RunbookStore::new();

    // Non-existent → NotFound
    let fake_rb = CompiledRunbook::new(Uuid::new_v4(), 1, vec![], ReplayEnvelope::empty());
    let fake_id = fake_rb.id;
    let result = execute_runbook(&store, fake_id, None, &SuccessExecutor).await;
    assert!(matches!(result, Err(ExecutionError::NotFound(_))));

    // Execute → re-execute → NotExecutable
    let step = make_step("cbu.create");
    let rb = CompiledRunbook::new(Uuid::new_v4(), 1, vec![step], ReplayEnvelope::empty());
    let id = rb.id;
    store.insert(&rb).await.unwrap();

    let result = execute_runbook(&store, id, None, &SuccessExecutor)
        .await
        .unwrap();
    assert!(matches!(
        result.final_status,
        CompiledRunbookStatus::Completed { .. }
    ));

    let retry = execute_runbook(&store, id, None, &SuccessExecutor).await;
    assert!(matches!(retry, Err(ExecutionError::NotExecutable(_, _))));
}
