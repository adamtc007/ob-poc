//! Runbook Pipeline Integration Tests
//!
//! Canonical harness exercising the full `compile_invocation` → `execute_runbook`
//! path. All integration tests for the runbook compilation + execution pipeline
//! live here.
//!
//! ## Coverage Matrix (Spec Refs)
//!
//! | Test                              | Spec ref                |
//! |-----------------------------------|-------------------------|
//! | `macro_end_to_end`                | §8.1, §11.1            |
//! | `pack_scoping`                    | §6, §7.3, §8.2         |
//! | `pack_completion_widening`        | §6.3                    |
//! | `replay_determinism`              | §9, INV-2              |
//! | `locking_no_deadlock`             | §10.2                   |
//! | `execution_gate_rejects_raw`      | §11.1, INV-3           |
//! | `constraint_violation_remediation` | §11.3                  |
//! | `runbook_immutability`            | §1, INV-1a             |

#![cfg(feature = "vnext-repl")]

use std::collections::{HashMap, HashSet};
use uuid::Uuid;

use ob_poc::dsl_v2::macros::{
    ArgStyle, MacroArg, MacroArgType, MacroArgs, MacroExpansionStep, MacroKind, MacroRegistry,
    MacroRouting, MacroSchema, MacroTarget, MacroUi, VerbCallStep,
};
use ob_poc::journey::pack_manager::{ConstraintSource, EffectiveConstraints};
use ob_poc::repl::verb_config_index::VerbConfigIndex;
use ob_poc::runbook::verb_classifier::{classify_verb, VerbClassification};
use ob_poc::runbook::{
    compile_verb, execute_runbook, CompiledRunbook, CompiledRunbookStatus, CompiledStep,
    ExecutionMode, OrchestratorResponse, ParkReason, ReplayEnvelope, RunbookStore, StepCursor,
    StepExecutor, StepOutcome,
};
use ob_poc::session::unified::{ClientRef, StructureType, UnifiedSession};

// =============================================================================
// Test helpers — canonical harness components
// =============================================================================

/// Create a minimal test session with a client in scope.
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

/// Build a `MacroRegistry` with a single `structure.setup` macro that expands
/// to `cbu.create`.
fn macro_registry_with_structure_setup() -> MacroRegistry {
    let mut registry = MacroRegistry::new();

    let required = {
        let mut m = HashMap::new();
        m.insert(
            "name".to_string(),
            MacroArg {
                arg_type: MacroArgType::Str,
                ui_label: "Name".to_string(),
                autofill_from: None,
                picker: None,
                default: None,
                valid_values: vec![],
                values: vec![],
                default_key: None,
                item_type: None,
                internal: None,
                required_if: None,
                placeholder_if_missing: false,
            },
        );
        m
    };

    registry.add(
        "structure.setup".to_string(),
        MacroSchema {
            id: None,
            kind: MacroKind::Macro,
            tier: None,
            aliases: vec![],
            taxonomy: None,
            ui: MacroUi {
                label: "Set up Structure".to_string(),
                description: "Create a new structure".to_string(),
                target_label: "Structure".to_string(),
            },
            routing: MacroRouting {
                mode_tags: vec![],
                operator_domain: Some("structure".to_string()),
            },
            target: MacroTarget {
                operates_on: "client-ref".to_string(),
                produces: Some("structure-ref".to_string()),
                allowed_structure_types: vec![],
            },
            args: MacroArgs {
                style: ArgStyle::Keyworded,
                required,
                optional: Default::default(),
            },
            required_roles: vec![],
            optional_roles: vec![],
            docs_bundle: None,
            prereqs: vec![],
            expands_to: vec![MacroExpansionStep::VerbCall(VerbCallStep {
                verb: "cbu.create".to_string(),
                args: {
                    let mut m = HashMap::new();
                    m.insert("name".to_string(), "${arg.name}".to_string());
                    m.insert("client-id".to_string(), "${scope.client_id}".to_string());
                    m
                },
                bind_as: None,
            })],
            sets_state: vec![],
            unlocks: vec![],
        },
    );
    registry
}

/// Build a multi-step macro registry (structure.setup expands to 2 steps).
fn macro_registry_with_multi_step() -> MacroRegistry {
    let mut registry = MacroRegistry::new();

    let required = {
        let mut m = HashMap::new();
        m.insert(
            "name".to_string(),
            MacroArg {
                arg_type: MacroArgType::Str,
                ui_label: "Name".to_string(),
                autofill_from: None,
                picker: None,
                default: None,
                valid_values: vec![],
                values: vec![],
                default_key: None,
                item_type: None,
                internal: None,
                required_if: None,
                placeholder_if_missing: false,
            },
        );
        m
    };

    registry.add(
        "structure.full-setup".to_string(),
        MacroSchema {
            id: None,
            kind: MacroKind::Macro,
            tier: None,
            aliases: vec![],
            taxonomy: None,
            ui: MacroUi {
                label: "Full Structure Setup".to_string(),
                description: "Create structure and profile".to_string(),
                target_label: "Structure".to_string(),
            },
            routing: MacroRouting {
                mode_tags: vec![],
                operator_domain: Some("structure".to_string()),
            },
            target: MacroTarget {
                operates_on: "client-ref".to_string(),
                produces: Some("structure-ref".to_string()),
                allowed_structure_types: vec![],
            },
            args: MacroArgs {
                style: ArgStyle::Keyworded,
                required,
                optional: Default::default(),
            },
            required_roles: vec![],
            optional_roles: vec![],
            docs_bundle: None,
            prereqs: vec![],
            expands_to: vec![
                MacroExpansionStep::VerbCall(VerbCallStep {
                    verb: "cbu.create".to_string(),
                    args: {
                        let mut m = HashMap::new();
                        m.insert("name".to_string(), "${arg.name}".to_string());
                        m
                    },
                    bind_as: Some("@cbu".to_string()),
                }),
                MacroExpansionStep::VerbCall(VerbCallStep {
                    verb: "trading-profile.create".to_string(),
                    args: {
                        let mut m = HashMap::new();
                        m.insert("cbu-id".to_string(), "@cbu".to_string());
                        m
                    },
                    bind_as: None,
                }),
            ],
            sets_state: vec![],
            unlocks: vec![],
        },
    );
    registry
}

/// Stub executor that always succeeds.
struct SuccessExecutor;

#[async_trait::async_trait]
impl StepExecutor for SuccessExecutor {
    async fn execute_step(&self, _step: &CompiledStep) -> StepOutcome {
        StepOutcome::Completed {
            result: serde_json::json!({"ok": true}),
        }
    }
}

/// Stub executor that fails on a specific verb.
struct FailOnVerb(String);

#[async_trait::async_trait]
impl StepExecutor for FailOnVerb {
    async fn execute_step(&self, step: &CompiledStep) -> StepOutcome {
        if step.verb == self.0 {
            StepOutcome::Failed {
                error: format!("{} failed", step.verb),
            }
        } else {
            StepOutcome::Completed {
                result: serde_json::json!({"ok": true}),
            }
        }
    }
}

/// Stub executor that parks on a specific verb.
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
            StepOutcome::Completed {
                result: serde_json::json!({"ok": true}),
            }
        }
    }
}

/// Stub executor that records which verbs were executed (for determinism tests).
struct RecordingExecutor {
    executed: std::sync::Mutex<Vec<String>>,
}

impl RecordingExecutor {
    fn new() -> Self {
        Self {
            executed: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn executed_verbs(&self) -> Vec<String> {
        self.executed.lock().unwrap().clone()
    }
}

#[async_trait::async_trait]
impl StepExecutor for RecordingExecutor {
    async fn execute_step(&self, step: &CompiledStep) -> StepOutcome {
        self.executed.lock().unwrap().push(step.verb.clone());
        StepOutcome::Completed {
            result: serde_json::json!({"ok": true}),
        }
    }
}

fn make_step(verb: &str) -> CompiledStep {
    CompiledStep {
        step_id: Uuid::new_v4(),
        sentence: format!("Do {verb}"),
        verb: verb.into(),
        dsl: format!("({verb})"),
        args: std::collections::BTreeMap::new(),
        depends_on: vec![],
        execution_mode: ExecutionMode::Sync,
        write_set: vec![],
    }
}

/// Helper: compile + execute a macro through the full pipeline.
async fn compile_and_execute(
    macro_name: &str,
    args: HashMap<String, String>,
    registry: &MacroRegistry,
    constraints: &EffectiveConstraints,
) -> (
    OrchestratorResponse,
    Option<ob_poc::runbook::executor::RunbookExecutionResult>,
) {
    let session = test_session();
    let verb_index = VerbConfigIndex::empty();
    let classification = classify_verb(macro_name, &verb_index, registry);

    let resp = compile_verb(
        Uuid::new_v4(),
        &classification,
        &args,
        &session,
        registry,
        1,
        constraints,
        None, // sem_reg_allowed_verbs
    );

    if let OrchestratorResponse::Compiled(ref summary) = resp {
        // Build a runbook from the summary and execute it
        let steps: Vec<_> = summary.preview.iter().map(|p| make_step(&p.verb)).collect();
        let rb = CompiledRunbook::new(Uuid::new_v4(), 1, steps, ReplayEnvelope::empty());
        let id = rb.id;
        let store = RunbookStore::new();
        store.insert(rb);

        let result = execute_runbook(&store, id, None, &SuccessExecutor)
            .await
            .unwrap();
        (resp, Some(result))
    } else {
        (resp, None)
    }
}

// =============================================================================
// Test 1: macro_end_to_end (§8.1, §11.1)
// =============================================================================

/// Verify that a macro utterance compiles through the classification →
/// expansion → constraint gate → step building pipeline, then executes
/// through the execution gate to completion.
#[tokio::test]
async fn macro_end_to_end() {
    let registry = macro_registry_with_structure_setup();
    let constraints = EffectiveConstraints::unconstrained();

    let mut args = HashMap::new();
    args.insert("name".to_string(), "Acme Fund".to_string());

    let (resp, exec_result) =
        compile_and_execute("structure.setup", args, &registry, &constraints).await;

    // Compilation succeeded
    assert!(resp.is_compiled(), "Expected Compiled, got {:?}", resp);
    let summary = match &resp {
        OrchestratorResponse::Compiled(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(summary.step_count, 1);
    assert_eq!(summary.preview[0].verb, "cbu.create");

    // Execution completed
    let result = exec_result.unwrap();
    assert!(matches!(
        result.final_status,
        CompiledRunbookStatus::Completed { .. }
    ));
    assert_eq!(result.step_results.len(), 1);
    assert!(matches!(
        result.step_results[0].outcome,
        StepOutcome::Completed { .. }
    ));
}

// =============================================================================
// Test 2: pack_scoping (§6, §7.3, §8.2)
// =============================================================================

/// Verify that pack constraints scope verb selection. When a pack allows
/// only specific verbs, the constraint gate blocks verbs outside the set.
#[tokio::test]
async fn pack_scoping() {
    let registry = macro_registry_with_structure_setup();

    // Pack allows only kyc verbs — cbu.create (from macro expansion) should be blocked
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

    let mut args = HashMap::new();
    args.insert("name".to_string(), "Acme Fund".to_string());

    let (resp, exec_result) =
        compile_and_execute("structure.setup", args, &registry, &constraints).await;

    // Compilation should produce a ConstraintViolation
    assert!(
        matches!(resp, OrchestratorResponse::ConstraintViolation(_)),
        "Expected ConstraintViolation, got {:?}",
        resp
    );

    if let OrchestratorResponse::ConstraintViolation(detail) = &resp {
        assert!(detail.violating_verbs.contains(&"cbu.create".to_string()));
        assert!(!detail.remediation_options.is_empty());
    }

    // No execution should have occurred
    assert!(exec_result.is_none());
}

// =============================================================================
// Test 3: pack_completion_widening (§6.3)
// =============================================================================

/// When a pack completes, its constraints are removed from the effective set.
/// This test verifies that a verb blocked by Pack A becomes available after
/// Pack A completes (INV-1a: completion widening doesn't affect the currently
/// executing runbook).
#[tokio::test]
async fn pack_completion_widening() {
    use ob_poc::journey::pack::PackManifest;
    use ob_poc::journey::pack_manager::PackManager;

    // Set up a PackManager with a restrictive pack
    let mut manager = PackManager::new();
    let pack_id = "kyc-case";

    // Register a manifest with restricted allowed_verbs
    let manifest = PackManifest {
        id: pack_id.to_string(),
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
    };
    manager.register_pack(manifest);
    manager.activate_pack(pack_id).unwrap();

    // Verify constraints are active (cbu.create blocked)
    let constraints_before = manager.effective_constraints();
    assert!(constraints_before.allowed_verbs.is_some());
    let allowed_before = constraints_before.allowed_verbs.as_ref().unwrap();
    assert!(!allowed_before.contains("cbu.create"));

    // Complete the pack
    manager.complete_pack(pack_id).unwrap();

    // Verify constraints widened (no packs → unconstrained)
    let constraints_after = manager.effective_constraints();
    assert!(
        constraints_after.allowed_verbs.is_none(),
        "After pack completion, should be unconstrained"
    );

    // Now cbu.create should compile successfully
    let registry = macro_registry_with_structure_setup();
    let mut args = HashMap::new();
    args.insert("name".to_string(), "Widened Fund".to_string());

    let (resp, exec_result) =
        compile_and_execute("structure.setup", args, &registry, &constraints_after).await;

    assert!(
        resp.is_compiled(),
        "Expected Compiled after widening, got {:?}",
        resp
    );
    let result = exec_result.unwrap();
    assert!(matches!(
        result.final_status,
        CompiledRunbookStatus::Completed { .. }
    ));
}

// =============================================================================
// Test 4: replay_determinism (§9, INV-2)
// =============================================================================

/// Verify that compiling the same utterance with the same inputs produces
/// identical runbook structure (step verbs, DSL, step count). The
/// ReplayEnvelope captures the snapshot for deterministic replay.
#[tokio::test]
async fn replay_determinism() {
    let registry = macro_registry_with_multi_step();
    let session = test_session();
    let verb_index = VerbConfigIndex::empty();
    let constraints = EffectiveConstraints::unconstrained();

    let mut args = HashMap::new();
    args.insert("name".to_string(), "Determinism Test".to_string());

    let classification = classify_verb("structure.full-setup", &verb_index, &registry);

    // Compile twice
    let resp1 = compile_verb(
        Uuid::new_v4(),
        &classification,
        &args,
        &session,
        &registry,
        1,
        &constraints,
        None, // sem_reg_allowed_verbs
    );
    let resp2 = compile_verb(
        Uuid::new_v4(),
        &classification,
        &args,
        &session,
        &registry,
        1,
        &constraints,
        None, // sem_reg_allowed_verbs
    );

    // Both should compile
    assert!(resp1.is_compiled());
    assert!(resp2.is_compiled());

    let s1 = match resp1 {
        OrchestratorResponse::Compiled(s) => s,
        _ => unreachable!(),
    };
    let s2 = match resp2 {
        OrchestratorResponse::Compiled(s) => s,
        _ => unreachable!(),
    };

    // Same structure: same step count, same verb order
    assert_eq!(s1.step_count, s2.step_count);
    assert_eq!(s1.preview.len(), s2.preview.len());
    for (a, b) in s1.preview.iter().zip(s2.preview.iter()) {
        assert_eq!(a.verb, b.verb, "Verb order must be deterministic");
    }

    // Execute both and verify same verb execution order
    let store1 = RunbookStore::new();
    let steps1: Vec<_> = s1.preview.iter().map(|p| make_step(&p.verb)).collect();
    let rb1 = CompiledRunbook::new(Uuid::new_v4(), 1, steps1, ReplayEnvelope::empty());
    let id1 = rb1.id;
    store1.insert(rb1);

    let store2 = RunbookStore::new();
    let steps2: Vec<_> = s2.preview.iter().map(|p| make_step(&p.verb)).collect();
    let rb2 = CompiledRunbook::new(Uuid::new_v4(), 1, steps2, ReplayEnvelope::empty());
    let id2 = rb2.id;
    store2.insert(rb2);

    let recorder1 = RecordingExecutor::new();
    let recorder2 = RecordingExecutor::new();

    let _ = execute_runbook(&store1, id1, None, &recorder1)
        .await
        .unwrap();
    let _ = execute_runbook(&store2, id2, None, &recorder2)
        .await
        .unwrap();

    assert_eq!(
        recorder1.executed_verbs(),
        recorder2.executed_verbs(),
        "Execution order must be deterministic"
    );
}

// =============================================================================
// Test 5: locking_no_deadlock (§10.2)
// =============================================================================

/// Verify that the write set computation produces a sorted (BTreeSet) lock
/// order, preventing deadlocks. Two runbooks with overlapping write sets
/// must produce the same lock acquisition order.
#[tokio::test]
async fn locking_no_deadlock() {
    let id_a = Uuid::parse_str("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa").unwrap();
    let id_b = Uuid::parse_str("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb").unwrap();
    let id_c = Uuid::parse_str("cccccccc-cccc-cccc-cccc-cccccccccccc").unwrap();

    // Runbook 1: steps reference entities in order [C, A, B]
    let mut step1 = make_step("cbu.create");
    step1.write_set = vec![id_c, id_a];
    let mut step2 = make_step("entity.create");
    step2.write_set = vec![id_b, id_a];

    // Runbook 2: steps reference entities in order [B, C, A]
    let mut step3 = make_step("cbu.update");
    step3.write_set = vec![id_b, id_c];
    let mut step4 = make_step("entity.update");
    step4.write_set = vec![id_a];

    // Compute write sets
    let ws1 = ob_poc::runbook::executor::compute_write_set(&[step1, step2], None);
    let ws2 = ob_poc::runbook::executor::compute_write_set(&[step3, step4], None);

    // Both should contain all three entity IDs
    assert_eq!(ws1.len(), 3);
    assert_eq!(ws2.len(), 3);

    // BTreeSet ensures sorted order — both produce [A, B, C]
    let order1: Vec<Uuid> = ws1.into_iter().collect();
    let order2: Vec<Uuid> = ws2.into_iter().collect();
    assert_eq!(
        order1, order2,
        "Lock order must be deterministic regardless of step order"
    );
    assert_eq!(order1[0], id_a, "Smallest UUID first");
    assert_eq!(order1[1], id_b);
    assert_eq!(order1[2], id_c);
}

// =============================================================================
// Test 6: execution_gate_rejects_raw (§11.1, INV-3)
// =============================================================================

/// Verify that `execute_runbook` enforces INV-3: no DSL may be executed
/// without a valid `CompiledRunbookId`. Attempting to execute a non-existent
/// runbook or a completed runbook must fail.
#[tokio::test]
async fn execution_gate_rejects_raw() {
    let store = RunbookStore::new();

    // 1. Non-existent runbook → NotFound
    // Build a content-addressed ID from dummy data that won't be in the store.
    let fake_rb = CompiledRunbook::new(Uuid::new_v4(), 1, vec![], ReplayEnvelope::empty());
    let fake_id = fake_rb.id;
    let result = execute_runbook(&store, fake_id, None, &SuccessExecutor).await;
    assert!(
        matches!(
            result,
            Err(ob_poc::runbook::executor::ExecutionError::NotFound(_))
        ),
        "Must reject non-existent runbook"
    );

    // 2. Execute a runbook to completion
    let rb = CompiledRunbook::new(
        Uuid::new_v4(),
        1,
        vec![make_step("cbu.create")],
        ReplayEnvelope::empty(),
    );
    let id = rb.id;
    store.insert(rb);

    let result = execute_runbook(&store, id, None, &SuccessExecutor)
        .await
        .unwrap();
    assert!(matches!(
        result.final_status,
        CompiledRunbookStatus::Completed { .. }
    ));

    // 3. Attempt to re-execute completed runbook → NotExecutable
    let result = execute_runbook(&store, id, None, &SuccessExecutor).await;
    assert!(
        matches!(
            result,
            Err(ob_poc::runbook::executor::ExecutionError::NotExecutable(
                _,
                _
            ))
        ),
        "Must reject already-completed runbook"
    );

    // 4. Manually mark a runbook as Failed, verify it's not executable
    let rb2 = CompiledRunbook::new(
        Uuid::new_v4(),
        2,
        vec![make_step("entity.create")],
        ReplayEnvelope::empty(),
    );
    let id2 = rb2.id;
    store.insert(rb2);

    let result = execute_runbook(&store, id2, None, &FailOnVerb("entity.create".into()))
        .await
        .unwrap();
    assert!(matches!(
        result.final_status,
        CompiledRunbookStatus::Failed { .. }
    ));

    // Re-execute failed runbook → NotExecutable
    let result = execute_runbook(&store, id2, None, &SuccessExecutor).await;
    assert!(
        matches!(
            result,
            Err(ob_poc::runbook::executor::ExecutionError::NotExecutable(
                _,
                _
            ))
        ),
        "Must reject failed runbook"
    );
}

// =============================================================================
// Test 7: constraint_violation_remediation (§11.3)
// =============================================================================

/// Verify that constraint violations produce actionable remediation options.
/// The ConstraintViolationDetail must include: the violating verbs, the
/// active constraints, and at least one remediation option.
#[tokio::test]
async fn constraint_violation_remediation() {
    let registry = macro_registry_with_structure_setup();

    // Constrain to session verbs only
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

    let mut args = HashMap::new();
    args.insert("name".to_string(), "Blocked Fund".to_string());

    let resp = compile_verb(
        Uuid::new_v4(),
        &classification,
        &args,
        &session,
        &registry,
        1,
        &constraints,
        None, // sem_reg_allowed_verbs
    );

    // Must be a ConstraintViolation
    let detail = match resp {
        OrchestratorResponse::ConstraintViolation(d) => d,
        other => panic!("Expected ConstraintViolation, got {:?}", other),
    };

    // Violating verbs identified
    assert!(
        !detail.violating_verbs.is_empty(),
        "Must report which verbs violated constraints"
    );
    assert!(detail.violating_verbs.contains(&"cbu.create".to_string()));

    // Active constraints reported
    assert!(
        !detail.active_constraints.is_empty(),
        "Must report which packs imposed constraints"
    );

    // Remediation options provided
    assert!(
        !detail.remediation_options.is_empty(),
        "Must offer at least one remediation option"
    );
}

// =============================================================================
// Test 8: runbook_immutability (§1, INV-1a)
// =============================================================================

/// Verify that once a CompiledRunbook is frozen, its steps cannot be modified
/// through execution. The runbook retrieved after execution must have the same
/// step count and step IDs as the original. Only the status field changes.
#[tokio::test]
async fn runbook_immutability() {
    let store = RunbookStore::new();

    let step1 = make_step("cbu.create");
    let step2 = make_step("entity.create");
    let step3 = make_step("trading-profile.create");
    let original_step_ids: Vec<Uuid> = vec![step1.step_id, step2.step_id, step3.step_id];
    let original_step_count = 3;

    let rb = CompiledRunbook::new(
        Uuid::new_v4(),
        1,
        vec![step1, step2, step3],
        ReplayEnvelope::empty(),
    );
    let id = rb.id;
    let original_version = rb.version;
    let original_session_id = rb.session_id;
    store.insert(rb);

    // Snapshot before execution
    let before = store.get(&id).unwrap();
    assert_eq!(before.steps.len(), original_step_count);
    assert!(matches!(before.status, CompiledRunbookStatus::Compiled));

    // Execute
    let result = execute_runbook(&store, id, None, &SuccessExecutor)
        .await
        .unwrap();
    assert!(matches!(
        result.final_status,
        CompiledRunbookStatus::Completed { .. }
    ));

    // Snapshot after execution
    let after = store.get(&id).unwrap();

    // Steps must be unchanged
    assert_eq!(
        after.steps.len(),
        original_step_count,
        "Step count must not change after execution"
    );
    for (i, step) in after.steps.iter().enumerate() {
        assert_eq!(
            step.step_id, original_step_ids[i],
            "Step IDs must not change after execution"
        );
    }

    // Metadata must be unchanged
    assert_eq!(after.version, original_version);
    assert_eq!(after.session_id, original_session_id);
    assert_eq!(after.id, id);

    // Only status changed
    assert!(matches!(
        after.status,
        CompiledRunbookStatus::Completed { .. }
    ));
}

// =============================================================================
// Additional pipeline tests — full path exercises
// =============================================================================

/// Multi-step macro: classify → expand → assemble → execute.
#[tokio::test]
async fn multi_step_macro_pipeline() {
    let registry = macro_registry_with_multi_step();
    let constraints = EffectiveConstraints::unconstrained();

    let mut args = HashMap::new();
    args.insert("name".to_string(), "Multi-Step Fund".to_string());

    let (resp, exec_result) =
        compile_and_execute("structure.full-setup", args, &registry, &constraints).await;

    assert!(resp.is_compiled());
    let summary = match &resp {
        OrchestratorResponse::Compiled(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(summary.step_count, 2);
    assert_eq!(summary.preview[0].verb, "cbu.create");
    assert_eq!(summary.preview[1].verb, "trading-profile.create");

    let result = exec_result.unwrap();
    assert!(matches!(
        result.final_status,
        CompiledRunbookStatus::Completed { .. }
    ));
    assert_eq!(result.step_results.len(), 2);
}

/// Parking mid-execution preserves runbook for resume.
#[tokio::test]
async fn park_and_resume_pipeline() {
    let store = RunbookStore::new();

    let step1 = make_step("cbu.create");
    let step2 = make_step("doc.solicit");
    let step2_id = step2.step_id;
    let step3 = make_step("entity.create");

    let rb = CompiledRunbook::new(
        Uuid::new_v4(),
        1,
        vec![step1, step2, step3],
        ReplayEnvelope::empty(),
    );
    let id = rb.id;
    store.insert(rb);

    // Execute — parks at step 2
    let result = execute_runbook(&store, id, None, &ParkOnVerb("doc.solicit".into()))
        .await
        .unwrap();
    assert!(matches!(
        result.final_status,
        CompiledRunbookStatus::Parked { .. }
    ));

    // Set status for resume
    store
        .update_status(
            &id,
            CompiledRunbookStatus::Parked {
                reason: ParkReason::AwaitingCallback {
                    correlation_key: "callback-1".into(),
                },
                cursor: StepCursor {
                    index: 1,
                    step_id: step2_id,
                },
            },
        )
        .unwrap();

    // Resume from step 2
    let cursor = StepCursor {
        index: 1,
        step_id: step2_id,
    };
    let result = execute_runbook(&store, id, Some(cursor), &SuccessExecutor)
        .await
        .unwrap();
    assert!(matches!(
        result.final_status,
        CompiledRunbookStatus::Completed { .. }
    ));
}

/// Primitive verb compiles to a single-step runbook without macro expansion.
#[tokio::test]
async fn primitive_verb_compile_and_execute() {
    let registry = MacroRegistry::new();
    let session = test_session();
    let constraints = EffectiveConstraints::unconstrained();

    let classification = VerbClassification::Primitive {
        fqn: "cbu.create".to_string(),
    };

    let mut args = HashMap::new();
    args.insert("name".to_string(), "Direct Fund".to_string());

    let resp = compile_verb(
        Uuid::new_v4(),
        &classification,
        &args,
        &session,
        &registry,
        1,
        &constraints,
        None, // sem_reg_allowed_verbs
    );

    assert!(resp.is_compiled());
    let summary = match &resp {
        OrchestratorResponse::Compiled(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(summary.step_count, 1);
    assert_eq!(summary.preview[0].verb, "cbu.create");

    // Execute
    let store = RunbookStore::new();
    let steps: Vec<_> = summary.preview.iter().map(|p| make_step(&p.verb)).collect();
    let rb = CompiledRunbook::new(Uuid::new_v4(), 1, steps, ReplayEnvelope::empty());
    let id = rb.id;
    store.insert(rb);

    let result = execute_runbook(&store, id, None, &SuccessExecutor)
        .await
        .unwrap();
    assert!(matches!(
        result.final_status,
        CompiledRunbookStatus::Completed { .. }
    ));
}

// =============================================================================
// Test: compile_invocation missing macro args → Clarification
// =============================================================================

/// When a macro verb is invoked with incomplete required args, the
/// `compile_invocation` function should return a Clarification with
/// the missing field names.
#[test]
fn compile_invocation_missing_macro_args() {
    let registry = macro_registry_with_structure_setup();
    let verb_index = VerbConfigIndex::empty();
    let session = test_session();
    let constraints = EffectiveConstraints::unconstrained();

    // Call compile_invocation with NO args — "name" is required by the macro
    let args = HashMap::new();

    let resp = ob_poc::runbook::compile_invocation(
        Uuid::new_v4(),
        "structure.setup",
        &args,
        &session,
        &registry,
        &verb_index,
        &constraints,
        1,
        None, // sem_reg_allowed_verbs
    );

    assert!(
        matches!(resp, OrchestratorResponse::Clarification(_)),
        "Expected Clarification for missing args, got {:?}",
        resp
    );

    if let OrchestratorResponse::Clarification(c) = &resp {
        // The clarification should mention the missing "name" field
        assert!(
            !c.missing_fields.is_empty(),
            "Clarification must list missing fields"
        );
        let field_names: Vec<&str> = c
            .missing_fields
            .iter()
            .map(|f| f.field_name.as_str())
            .collect();
        assert!(
            field_names.contains(&"name"),
            "Missing fields must include 'name', got {:?}",
            field_names
        );
    }
}

// =============================================================================
// Test: compile_invocation end-to-end with complete args
// =============================================================================

/// Verify that `compile_invocation` with complete args produces a Compiled
/// response, confirming the full classify → compile pipeline works.
#[test]
fn compile_invocation_end_to_end() {
    let registry = macro_registry_with_structure_setup();
    let verb_index = VerbConfigIndex::empty();
    let session = test_session();
    let constraints = EffectiveConstraints::unconstrained();

    let mut args = HashMap::new();
    args.insert("name".to_string(), "E2E Fund".to_string());

    let resp = ob_poc::runbook::compile_invocation(
        Uuid::new_v4(),
        "structure.setup",
        &args,
        &session,
        &registry,
        &verb_index,
        &constraints,
        1,
        None, // sem_reg_allowed_verbs
    );

    assert!(
        resp.is_compiled(),
        "Expected Compiled for complete args, got {:?}",
        resp
    );

    let summary = match &resp {
        OrchestratorResponse::Compiled(s) => s,
        _ => unreachable!(),
    };
    assert_eq!(summary.step_count, 1);
    assert_eq!(summary.preview[0].verb, "cbu.create");
}

// =============================================================================
// Test: compile_invocation with constraint violation
// =============================================================================

/// Verify that `compile_invocation` returns ConstraintViolation when the
/// expanded verb is outside the pack's allowed set.
#[test]
fn compile_invocation_constraint_violation() {
    let registry = macro_registry_with_structure_setup();
    let verb_index = VerbConfigIndex::empty();
    let session = test_session();

    // Only allow KYC verbs — cbu.create (from macro expansion) should be blocked
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

    let mut args = HashMap::new();
    args.insert("name".to_string(), "Blocked Fund".to_string());

    let resp = ob_poc::runbook::compile_invocation(
        Uuid::new_v4(),
        "structure.setup",
        &args,
        &session,
        &registry,
        &verb_index,
        &constraints,
        1,
        None, // sem_reg_allowed_verbs
    );

    assert!(
        matches!(resp, OrchestratorResponse::ConstraintViolation(_)),
        "Expected ConstraintViolation, got {:?}",
        resp
    );

    if let OrchestratorResponse::ConstraintViolation(detail) = &resp {
        assert!(
            detail.violating_verbs.contains(&"cbu.create".to_string()),
            "Should report cbu.create as violating"
        );
    }
}
